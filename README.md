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

Der lokale Releasebuild verwendet [scripts/build-release.ps1](scripts/build-release.ps1),
lädt bei Bedarf automatisch die MSVC-Umgebung und signiert Updater-Artefakte mit
`%USERPROFILE%\.tauri\akex.key` und der daneben liegenden Passwortdatei.

## Releases und automatische Updates

Akex prüft bei jedem Start asynchron
`https://github.com/Akzuwo/AkEx/releases/latest/download/latest.json`. Eine
neuere, korrekt signierte Version wird heruntergeladen, im Hintergrund über
den NSIS-Current-User-Installer installiert und danach gestartet. Netzwerk- oder
Updatefehler werden nur protokolliert und blockieren den Programmstart nicht.

Der Workflow [release.yml](.github/workflows/release.yml) wird im GitHub-Actions-
Panel über **Run workflow** gestartet. Als Eingabe ist eine SemVer-Version wie
`0.2.0` erforderlich. Der Workflow synchronisiert alle Versionsfelder, führt
Frontend-Build und Rust-Tests aus und veröffentlicht MSI, NSIS, Signaturen sowie
`latest.json` als GitHub Release.

Vor dem ersten Release müssen privater Schlüssel und Passwort als Repository-
Secrets `TAURI_SIGNING_PRIVATE_KEY` und `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
hinterlegt werden. Mit installierter und authentifizierter GitHub CLI erledigt
dies:

```powershell
.\scripts\configure-github-updater-secret.ps1
```

Der private Schlüssel und sein Passwort liegen ausschließlich unter
`%USERPROFILE%\.tauri\akex.key` beziehungsweise `akex.key.password`. Beide
dürfen nicht ins Repository gelangen und müssen gemeinsam sicher gesichert
werden: Bei Verlust können bestehende Installationen keine neuen Updates mehr
verifizieren.

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
