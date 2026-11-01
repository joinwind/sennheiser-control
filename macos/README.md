# Sennheiser Control for macOS (Liquid Glass Edition)

Versione nativa per **macOS** di Sennheiser Control, progettata con un'interfaccia **Liquid Glass / Control Center** armonizzata per **macOS 27** e basata sullo stesso motore GAIA v3 (`crates/protocol` + `crates/engine`) e su `IOBluetooth.framework` (`crates/bluetooth/src/macos.rs`).

## Caratteristiche specifiche macOS

- **Design Liquid Glass (macOS 27)**:
  - Superfici traslucide multilivello con riflessi speculari sul bordo superiore (`1px` specular rim), bagliore ambientale dinamico sullo sfondo e geometria concentrica.
  - Semafori macOS (*Traffic Lights*) interattivi nella barra superiore, pillole animate in stile Control Center e slider a capsula spessa ispirati ai controlli Volume/Luminosità di macOS.
  - Tipografia nativa **San Francisco** (`/System/Library/Fonts/SFNS.ttf`) con fallback automatico.
- **Menu Bar App (`NSStatusItem`)**:
  - Gestita direttamente sul **Main Thread** (come richiesto da `AppKit` su macOS).
  - La finestra si apre come **popover sotto la Menu Bar in alto a destra**, in stile Control Center.
  - Con `LSUIElement = true` nell'`Info.plist`, l'app vive silenziosamente nella Menu Bar senza occupare spazio nel Dock quando la finestra è chiusa.
- **Bluetooth Classic RFCOMM nativo (`IOBluetooth.framework`)**:
  - Rilevamento automatico delle MOMENTUM 4 associate a macOS (`[IOBluetoothDevice pairedDevices]`).
  - Risoluzione SDP per l'UUID GAIA v3 e apertura del canale RFCOMM su un thread con `CFRunLoop` dedicato.
- **Integrazione macOS**:
  - Preferenze e log in `~/Library/Application Support/SennheiserControl/`.
  - **Open at Login** tramite LaunchAgent (`~/Library/LaunchAgents/com.sennheiser-control.app.plist`).
  - Single-instance tramite Unix Domain Socket (`instance.sock`).
  - Notifiche di batteria scarica tramite Notification Center di macOS.

## Compilazione ed esecuzione su macOS

Richiede Rust (`rustup`) e gli Xcode Command Line Tools (`xcode-select --install`).

```bash
cd macos

# Esecuzione in modalità simulata (senza cuffie fisiche)
cargo run -- --sim

# Compilazione e creazione del bundle "Sennheiser Control.app"
chmod +x scripts/bundle-app.sh
./scripts/bundle-app.sh

# Avvio dell'app impacchettata
open "target/release/bundle/Sennheiser Control.app"
```

> **Nota sui permessi Bluetooth di macOS**: Al primo avvio di `Sennheiser Control.app`, macOS mostrerà il popup di autorizzazione Bluetooth (configurato tramite `NSBluetoothAlwaysUsageDescription` in `Info.plist`).

## Flag da riga di comando

| Flag | Effetto |
|---|---|
| `--minimized` | Avvia solo nella Menu Bar senza aprire il popover (usato da *Open at Login*) |
| `--sim` | Usa la cuffia simulata (`settings-sim.json`) |
| `--quit` | Chiede all'istanza in esecuzione di chiudersi |
| `--debug` | Log verboso con tutti i frame GAIA in `~/Library/Application Support/SennheiserControl/app.log` |
| `--render-icons DIR` | Genera le icone `.iconset` per `iconutil` e le anteprime della Menu Bar |

