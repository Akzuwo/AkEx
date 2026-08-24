# Akex – Index-First File Explorer

Akex ist ein performanter Windows-Dateibrowser auf Basis von Rust, Tauri 2,
React, TypeScript und SQLite/FTS5. Navigation, Suche, Ordnergrössen und
Speicheranalyse lesen aus einem vorberechneten Index. Das reale Dateisystem
bleibt die autoritative Quelle für Dateioperationen.

## Entwicklung

Voraussetzungen sind Node.js, Rust (MSVC-Toolchain), Visual Studio Build Tools
mit „Desktop development with C++“ und WebView2.

```powershell
npm install
npm run tauri:dev
```

Tests und Produktionsbuild:

```powershell
npm run build
npm run test:rust
npm run tauri:build
```

Die SQLite-Datenbank liegt im von Tauri bereitgestellten App-Datenverzeichnis.
Migrationen befinden sich in `src-tauri/migrations`.

## Architektur

- `src-tauri/database`: Migrationen und sämtliche SQL-Zugriffe
- `src-tauri/indexer`: speicherschonender Streaming-Scan mit Bottom-up-Grössen
- `src-tauri/search`: Parser für Suchsyntax und strukturierte SQL-/FTS-Filter
- `src-tauri/filesystem`: reale Dateioperationen und Laufwerkerkennung
- `src-tauri/watcher`: inkrementelle Updates über `ChangeProvider`
- `src-tauri/commands`: schmale IPC-Grenze zur UI
- `src`: paginierte, virtualisierte React-Desktopoberfläche

## Aktueller Umfang

Implementiert sind Initialindex, FTS5-Suche, indexbasierte Navigation,
voraggregierte Ordnergrössen, Dateioperationen (inklusive Papierkorb),
Speicheranalyse, Indexprüfung, Fortschritt/Abbruch und ein rekursiver
Dateisystem-Watcher. Der USN-Journal-Provider ist als austauschbare Phase-9-
Implementierung vorbereitet und im Code ausdrücklich als noch inaktiv markiert.
# AkEx
