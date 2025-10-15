use bytes::Bytes;
use std::path::Path;
use std::{fs, io::Write};
use tokio::task;

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
