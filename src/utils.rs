use chrono::Datelike;
use bytes::Bytes;
use std::path::Path;
use std::{fs, io::Write};
use chrono::{Local, NaiveDate};
use image::codecs::gif::GifEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::{DynamicImage, imageops::FilterType, io::Reader as ImageReader};
use image::{ExtendedColorType, Frame, ImageEncoder};
use tokio::task;

/// Funzione per ripulire una stringa
/// Utile per slug o nomi dei files
///
/// # Argomenti
/// name -> string, nome del file
pub fn sanitize_name(name: &str) -> String {
    let mut s = name.trim().to_lowercase();

    let cerca = [
        "à", "è", "é", "ì", "ò", "ù", "'", "?", " ", "__", "&", "%", "#", "(", ")", "/", "+", "°",
    ];

    let sostituisci = [
        "a",
        "e",
        "e",
        "i",
        "o",
        "u",
        "-",
        "-",
        "-",
        "-",
        "e",
        "-per-cento-",
        "-",
        "",
        "",
        "-",
        "_",
        "_",
    ];

    for (c, r) in cerca.iter().zip(sostituisci.iter()) {
        s = s.replace(c, r);
    }

    s = s.replace("---", "-");

    s
}

/// Funzione per upload generico
///
/// # Argomenti
/// base_dir -> stringa che rappresenta la directory di upload; viene creata se non esiste
/// original_name -> nome del file caricato
/// file_bytes -> bytes inviati
/// allowed_types -> array con tipi di file permessi nell'upload
/// max_size -> dimensione massima di upload
pub async fn save_uploaded_file(
    base_dir: &str,
    original_name: &str,
    file_bytes: &Bytes,
    allowed_types: &[&str],
    max_size: usize,
) -> Result<String, String> {
    let ext = Path::new(&original_name)
        .extension()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "File senza estensione".to_string())?
        .to_lowercase();

    if !allowed_types.contains(&ext.as_str()) {
        return Err(format!("Tipo file non consentito: {}", ext));
    }

    if file_bytes.len() > max_size {
        return Err(format!(
            "File troppo grande: {} byte, massimo {} byte",
            file_bytes.len(),
            max_size
        ));
    }

    fs::create_dir_all(base_dir).map_err(|e| format!("Errore creazione cartella: {}", e))?;

    let save_path = format!("{}/{}", base_dir, original_name);
    let bytes = file_bytes.clone();

    task::spawn_blocking(move || -> Result<String, String> {
        let mut file =
            fs::File::create(&save_path).map_err(|e| format!("Errore creazione file: {}", e))?;
        file.write_all(&bytes)
            .map_err(|e| format!("Errore scrittura file: {}", e))?;
        Ok(save_path)
    })
    .await
    .map_err(|e| format!("Errore thread: {}", e))?
}

/// Funzione specifica per upload di immagini
/// Fa anche lo scaling in base a determinate regole
///
/// # Argomenti
/// base_dir -> stringa che rappresenta la directory di upload; viene creata se non esiste
/// original_name -> nome del file caricato
/// file_bytes -> bytes inviati
/// max_size -> dimensione massima di upload
/// width -> larghezza in base alla quale fare il resizing
/// height -> altezza in base alla quale fare il resizing
pub async fn save_uploaded_image(
    base_dir: &str,
    original_name: &str,
    file_bytes: &Bytes,
    max_size: usize,
    width: i32,
    height: i32,
) -> Result<String, String> {
    if file_bytes.len() > max_size {
        return Err(format!(
            "File troppo grande: {} byte, massimo {} byte",
            file_bytes.len(),
            max_size
        ));
    }

    let ext = Path::new(original_name)
        .extension()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "File senza estensione".to_string())?
        .to_lowercase();

    let allowed_types = ["png", "jpg", "jpeg", "gif"];
    if !allowed_types.contains(&ext.as_str()) {
        return Err(format!("Tipo file non consentito: {}", ext));
    }

    fs::create_dir_all(base_dir).map_err(|e| format!("Errore creazione cartella: {}", e))?;

    let file_name = sanitize_name(original_name);
    let save_path = format!("{}/{}", base_dir, file_name);
    let bytes = file_bytes.clone();

    task::spawn_blocking(move || -> Result<String, String> {
        let img = ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()
            .map_err(|e| format!("Formato immagine non valido: {}", e))?
            .decode()
            .map_err(|e| format!("Errore decodifica immagine: {}", e))?;

        let (orig_w, orig_h) = (img.width(), img.height());

        let img: DynamicImage = match (width, height) {
            (w, 0) if w > 0 && orig_w > w as u32 => {
                // ridimensiona solo se la larghezza originale è maggiore
                let ratio = w as f32 / orig_w as f32;
                img.resize(
                    w as u32,
                    (orig_h as f32 * ratio) as u32,
                    FilterType::Lanczos3,
                )
            }
            (0, h) if h > 0 && orig_h > h as u32 => {
                // ridimensiona solo se l'altezza originale è maggiore
                let ratio = h as f32 / orig_h as f32;
                img.resize(
                    (orig_w as f32 * ratio) as u32,
                    h as u32,
                    FilterType::Lanczos3,
                )
            }
            (w, h) if w > 0 && h > 0 => {
                // forza scaling indipendentemente dalle proporzioni
                img.resize_exact(w as u32, h as u32, FilterType::Lanczos3)
            }
            _ => img, // nessuno scaling
        };

        let mut out_file =
            fs::File::create(&save_path).map_err(|e| format!("Errore creazione file: {}", e))?;

        match ext.as_str() {
            "png" => {
                let encoder = PngEncoder::new(&mut out_file);
                encoder
                    .write_image(
                        &img.to_rgba8(),
                        img.width(),
                        img.height(),
                        ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| format!("Errore scrittura PNG: {}", e))?;
            }
            "jpg" | "jpeg" => {
                let encoder = JpegEncoder::new(&mut out_file);
                encoder
                    .write_image(
                        &img.to_rgb8(),
                        img.width(),
                        img.height(),
                        ExtendedColorType::Rgb8,
                    )
                    .map_err(|e| format!("Errore scrittura JPEG: {}", e))?;
            }
            "gif" => {
                let mut encoder = GifEncoder::new(&mut out_file);
                let frame = Frame::new(img.to_rgba8());
                encoder
                    .encode_frame(frame)
                    .map_err(|e| format!("Errore scrittura GIF: {}", e))?;
            }
            _ => return Err("Formato non supportato".to_string()),
        };

        Ok(save_path)
    })
        .await
        .map_err(|e| format!("Errore thread: {}", e))?
}

/// Calcola i giorni presenti in un anno
fn days_in_year(year: i32) -> u32 {
    let mut days = 0;

    for month in 1..=12 {
        let first_of_month = NaiveDate::from_ymd_opt(year, month, 1).unwrap();

        let (next_year, next_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        let first_of_next_month = NaiveDate::from_ymd_opt(next_year, next_month, 1).unwrap();

        let days_in_month = (first_of_next_month - first_of_month).num_days() as u32;

        days += days_in_month;
    }

    days
}

/// Calcola i giorni passati dal primo gennaio dell'anno corrente
fn days_passed_from_start_year() -> i64 {
    let today = Local::now().date_naive();
    let start_of_year = NaiveDate::from_ymd_opt(today.year(), 1, 1).expect("Data non valida");
    (today - start_of_year).num_days()
}
