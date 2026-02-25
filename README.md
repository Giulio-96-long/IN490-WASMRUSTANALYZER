# WamRustAnalyzer

WebAssembly Rust Analyzer per estrazione di testo e immagini da file
HTML e PDF, eseguito interamente nel browser.

------------------------------------------------------------------------

## Descrizione del progetto

WamRustAnalyzer è un'applicazione web che utilizza:

-   Rust
-   WebAssembly (WASM)
-   pdfium-render
-   JavaScript (frontend puro)
-   scraper (HTML parser in Rust)
-   image (conversione immagini)

per estrarre:

-   Testo da file HTML
-   Testo da file PDF
-   Immagini da file HTML
-   Immagini embedded nei PDF

Tutto viene elaborato client-side, senza backend e senza inviare file a
server esterni.

------------------------------------------------------------------------

## Architettura del progetto

progetto/
│
├── src/
│   └── lib.rs              # Logica Rust -> WebAssembly
│
├── web/
│   ├── index.html          # Frontend
│   ├── pkg/                # Generato da wasm-pack
│   └── wasm/               # pdfium.js + file wasm di pdfium
│
├── Cargo.toml
└── README.md

------------------------------------------------------------------------

## Tecnologie utilizzate

  Tecnologia      Ruolo
  --------------- ---------------------------------
  Rust            Motore di parsing ed estrazione
  wasm-bindgen    Bridge Rust ↔ JS
  pdfium-render   Parsing PDF
  scraper         Parsing HTML
  image           Conversione RGBA → PNG
  http-server     Server statico locale

------------------------------------------------------------------------

## Come funziona

### HTML

1.  Il file HTML viene letto come stringa.
2.  Rust usa `scraper` per:
    -   Estrarre testo dal `<body>`
    -   Estrarre tutti gli `src` delle immagini
3.  I risultati vengono serializzati e restituiti al JS.

### PDF

1.  Il file viene letto come `Uint8Array`.
2.  Rust verifica che inizi con `%PDF-`.
3.  Pdfium carica il documento.
4.  Per ogni pagina:
    -   Estrae il testo.
    -   Analizza gli oggetti pagina.
    -   Se trova un'immagine:
        -   Ottiene i raw bytes.
        -   Prova a ricostruire le dimensioni (RGBA).
        -   Converte in PNG.
5.  Restituisce una risposta strutturata:

{ 
    "ok": true, 
    "data": 
        { 
            "text": "...",
            "images": \[...\] 
        } 
}

------------------------------------------------------------------------

## Installazione

### Requisiti

-   Rust
-   wasm-pack
-   Node.js
-   npm

Installa wasm-pack se necessario:

cargo install wasm-pack

------------------------------------------------------------------------

## Build del modulo WASM

Posizionati nella cartella del progetto:

wasm-pack build --target web --out-name wamrustanalyzer --out-dir
web/pkg --release

Questo genera nella cartella `web/pkg`:

-   wamrustanalyzer_bg.wasm
-   wamrustanalyzer.js
-   file di supporto generati automaticamente

------------------------------------------------------------------------

## Avvio del progetto

Spostati nella cartella `web`:

cd web

Avvia un server statico dalla cartella `web`:

npx http-server -c-1 -o

Se il comando non funziona, installare prima:

npm install -g http-server

------------------------------------------------------------------------

## Autore

Progetto sviluppato per studio e sperimentazione su Rust + WebAssembly.
