
// ============================================================
// PROGETTO: Rust + WebAssembly Document Analyzer
// ============================================================
//
// DESCRIZIONE GENERALE
//
// Il progetto consiste nello sviluppo di un sistema di analisi
// documentale realizzato in Rust e compilato in WebAssembly (WASM).
//
// L’obiettivo principale è consentire l’estrazione di testo e immagini
// da file HTML e PDF direttamente in ambiente browser, sfruttando:
//
// - Le prestazioni del linguaggio Rust
// - Il modello di sicurezza sandbox di WebAssembly
// - L’integrazione della libreria PDFium per l’analisi dei PDF
//
// L’intero processo di parsing ed estrazione avviene lato client,
// senza necessità di backend remoto.
//
// Il sistema permette di:
//
// - Estrarre il testo contenuto nel <body> dei file HTML
// - Raccogliere i valori dell’attributo "src" dei tag <img>
// - Analizzare documenti PDF nativi
// - Estrarre testo e immagini raster dai PDF
// - Restituire i risultati al frontend in formato JSON strutturato
//
// ------------------------------------------------------------
// COMPILAZIONE DEL PROGETTO
// ------------------------------------------------------------
//
// Posizionarsi nella cartella principale del progetto
//    (dove si trova Cargo.toml)
//
// Compilare il modulo WebAssembly eseguendo:
//
//    wasm-pack build --target web --out-name wamrustanalyzer --out-dir web/pkg --release
//
// Questo comando:
//
// - Compila il codice Rust in WebAssembly
// - Genera i binding JavaScript tramite wasm-bindgen
// - Crea/aggiorna la cartella "web/pkg"
//
// All’interno di web/pkg vengono generati:
//
// - wamrustanalyzer_bg.wasm  -> modulo WebAssembly compilato
// - wamrustanalyzer.js       -> wrapper JavaScript
// - file di supporto generati automaticamente
//
// ------------------------------------------------------------
// STRUTTURA NECESSARIA DEL FRONTEND
// ------------------------------------------------------------
//
// Nella cartella:
//
//    ../progetto_pdfium/web
//
// devono essere presenti:
//
// - index.html      -> interfaccia utente
// - pkg/            -> modulo WASM generato da wasm-pack
// - wasm/           -> runtime PDFium (pdfium.js + relativi .wasm)
//
// ------------------------------------------------------------
// AVVIO DELL'APPLICAZIONE
// ------------------------------------------------------------
//
// È necessario utilizzare un server HTTP statico,
// poiché WebAssembly non può essere eseguito correttamente
// tramite apertura diretta del file HTML (file://).
//
// Dalla cartella "web" eseguire:
//
//    http-server -c-1 -o
//
// In alternativa è possibile utilizzare qualsiasi altro
// server statico equivalente.
//
// Aprire quindi il browser all’indirizzo indicato
// (es: http://127.0.0.1:8080).
//
// Dall’interfaccia sarà possibile:
//
// - Caricare un file HTML
// - Caricare un file PDF
// - Visualizzare il testo estratto
// - Visualizzare le immagini estratte
//
// Tutta l’elaborazione avviene lato browser tramite WebAssembly.
//
// ============================================================

use wasm_bindgen::prelude::*;
use scraper::{Html, Selector};
use serde::{Serialize, Deserialize};
use serde_wasm_bindgen::to_value;

use pdfium_render::prelude::*;
use std::io::Cursor;
use web_sys::console;
use image::{DynamicImage, RgbaImage};

/// Errore standard restituito al frontend
#[derive(Serialize)]
struct ErrorInfo {
    code: String,     // es: "PDF_LOAD_FAIL"
    message: String,  // messaggio leggibile
}

/// Risposta standard restituita al frontend
///
/// Con skip_serializing_if:
/// - se data = None -> niente campo "data"
/// - se error = None -> niente campo "error"
#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    ok: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,

    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorInfo>,
}

/// Crea una risposta OK.
/// Ritorna un JsValue che wasm-bindgen espone come plain object JS.
fn js_ok<T: Serialize>(data: T) -> JsValue {
    let response = ApiResponse {
        ok: true,
        data: Some(data),
        error: None,
    };

    // Normalmente to_value non fallisce, ma per sicurezza teniamo traccia del value
    match to_value(&response) {
        Ok(v) => v,
        Err(e) => {
            console::log_1(&format!("Serializzazione OK fallita: {:?}", e).into());
            // Fallback minimale: stringa JSON.           
            JsValue::from_str(r#"{"ok":false,"error":{"code":"SERDE_OK_FAIL","message":"Errore interno"}} "#)
        }
    }
}

/// Crea una risposta di errore.
fn js_err(code: &str, message: &str) -> JsValue {
    // Notare: ApiResponse<()> perché in errore non abbiamo data.
    let response: ApiResponse<()> = ApiResponse {
        ok: false,
        data: None,
        error: Some(ErrorInfo {
            code: code.to_string(),
            message: message.to_string(),
        }),
    };

    match to_value(&response) {
        Ok(v) => v,
        Err(e) => {
            console::log_1(&format!("Serializzazione ERR fallita: {:?}", e).into());
            JsValue::from_str(r#"{"ok":false,"error":{"code":"SERDE_ERR_FAIL","message":"Errore interno"}} "#)
        }
    }
}

/// Risultato estrazione HTML (testo + lista src immagini)
#[derive(Serialize, Deserialize)]
struct ExtractionResultHTML {
    text: String,
    images: Vec<String>,
}

/// Estrazione contenuti HTML:
/// - Parse documento
/// - Estrai testo dal body
/// - Estrai src di tutte le img
#[wasm_bindgen]
pub fn extract_content_from_html(html: &str) -> JsValue {
    // Parser HTML lato Rust
    let document = Html::parse_document(html);

    // Selettore per <body>
    let body_selector = match Selector::parse("body") {
        Ok(sel) => sel,
        Err(e) => {
            console::log_1(&format!("Errore selector body: {:?}", e).into());
            return js_err("HTML_SELECTOR_BODY", "Errore interno: selector 'body' non valido");
        }
    };

    // Prendi il primo body e concatena tutti i nodi testo discendenti 
    let text = document
        .select(&body_selector)
        .next()
        .map(|body| body.text().collect::<String>())
        .unwrap_or_else(|| "Nessun testo trovato".to_string());

    // Selettore per <img>
    let img_selector = match Selector::parse("img") {
        Ok(sel) => sel,
        Err(e) => {
            console::log_1(&format!("Errore selector img: {:?}", e).into());
            return js_err("HTML_SELECTOR_IMG", "Errore interno: selector 'img' non valido");
        }
    };

    // Raccogliamo tutti gli src delle immagini (se presenti)
    let images: Vec<String> = document
        .select(&img_selector)
        .filter_map(|img| img.value().attr("src").map(String::from))
        .collect();

    js_ok(ExtractionResultHTML { text, images })
}

/// Risultato estrazione PDF:
/// - testo estratto
/// - immagini in formato PNG come Vec<u8> (che in JS diventa Uint8Array)
#[derive(Serialize)]
struct ExtractionResultPDF {
    text: String,
    images: Vec<Vec<u8>>,
}

/// Estrazione contenuti PDF:
/// - controllo “base” che inizi con %PDF-
/// - carico documento con pdfium-render
/// - per ogni pagina:
///   * estraggo testo
///   * cerco oggetti immagine e provo a convertirli in PNG
#[wasm_bindgen]
pub fn extract_content_from_pdf(pdf_data: Vec<u8>) -> JsValue {
    // Controllo veloce: filtra input ovviamente non-PDF
    if !pdf_data.starts_with(b"%PDF-") {
        return js_err("PDF_BAD_FORMAT", "Formato file non corretto: inserire un PDF valido");
    }

    let pdfium = Pdfium::default();

    let mut extracted_text = String::new();
    let mut extracted_images: Vec<Vec<u8>> = Vec::new();

    // Caricamento documento dal byte slice
    let document = match pdfium.load_pdf_from_byte_slice(&pdf_data, None) {
        Ok(doc) => doc,
        Err(e) => {
            console::log_1(&format!("Errore caricamento PDF: {:?}", e).into());
            return js_err("PDF_LOAD_FAIL", "Errore caricamento PDF");
        }
    };

    // Helper locale: prova a ricavare (w,h) assumendo RGBA (4 bytes per pixel)
    // Nota: è un'euristica. Se non è RGBA o se le dimensioni non sono deducibili, ritorna None.
    let guess_rgba_dimensions = |byte_len: usize| -> Option<(u32, u32)> {
        if byte_len % 4 != 0 {
            return None;
        }
        let pixels = byte_len / 4;
        if pixels == 0 {
            return None;
        }

        // Prova divisori vicini a sqrt(pixels): spesso trovi dimensioni "sensate" prima.
        let root = (pixels as f64).sqrt() as usize;

        // Limiti prudenziali per evitare immagini gigantesche che bloccano il browser
        let max_side: usize = 8192;

        // Scansiono verso il basso da sqrt(pixels) per trovare un divisore.
        // Se w divide pixels, allora h = pixels / w.
        let start = root.min(max_side).max(1);
        for w in (1..=start).rev() {
            if pixels % w == 0 {
                let h = pixels / w;
                if h <= max_side {
                    return Some((w as u32, h as u32));
                }
            }
        }

        // Fallback: scansiono verso l'alto (raro ma può servire)
        let end = (max_side).min(pixels);
        for w in start..=end {
            if pixels % w == 0 {
                let h = pixels / w;
                if h <= max_side {
                    return Some((w as u32, h as u32));
                }
            }
        }

        None
    };

    // Iterazione pagine
    for i in 0..document.pages().len() {
        let page = match document.pages().get(i) {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Estrazione testo
        if let Ok(text) = page.text() {
            extracted_text.push_str(&text.all());
            extracted_text.push('\n');
        }

        // Estrazione immagini
        for object in page.objects().iter() {
            if let Some(image_obj) = object.as_image_object() {
                // Provo a leggere l'immagine raw
                let raw_image = match image_obj.get_raw_image() {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let raw_bytes = raw_image.as_bytes();

                // 1) Prima prova: usare width/height dell'oggetto SOLO se tornano con i bytes RGBA
                // (PdfPoints non sono sempre pixel, ma se combaciano li prendiamo al volo)
                let w_hint = image_obj.width().map(|p| p.value as i64).unwrap_or(0);
                let h_hint = image_obj.height().map(|p| p.value as i64).unwrap_or(0);

                let mut dims: Option<(u32, u32)> = None;

                if w_hint > 0 && h_hint > 0 {
                    let (w, h) = (w_hint as u32, h_hint as u32);
                    let expected = (w as usize)
                        .saturating_mul(h as usize)
                        .saturating_mul(4);

                    if expected == raw_bytes.len() {
                        dims = Some((w, h));
                    }
                }

                // 2) Se non combaciano, faccio guess dai bytes
                let (width, height) = match dims.or_else(|| guess_rgba_dimensions(raw_bytes.len())) {
                    Some(d) => d,
                    None => continue, // non sembra RGBA o non deducibile
                };

                // 3) Costruisco immagine RGBA
                let rgba = match RgbaImage::from_raw(width, height, raw_bytes.to_vec()) {
                    Some(img) => img,
                    None => continue,
                };

                // 4) Converto in PNG bytes
                let dynamic_img = DynamicImage::ImageRgba8(rgba);
                let mut buffer: Vec<u8> = Vec::new();
                let mut cursor = Cursor::new(&mut buffer);

                if dynamic_img.write_to(&mut cursor, image::ImageFormat::Png).is_ok() && !buffer.is_empty() {
                    extracted_images.push(buffer);
                }
            }
        }
    }

    js_ok(ExtractionResultPDF {
        text: extracted_text,
        images: extracted_images,
    })
}
