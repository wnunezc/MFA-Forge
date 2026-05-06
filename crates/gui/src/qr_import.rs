pub fn decode_otpauth_uri_from_image(path: &str) -> Result<String, String> {
    if path.trim().is_empty() {
        return Err("Indica la ruta de una imagen QR antes de importar.".to_owned());
    }

    let image =
        image::open(path).map_err(|error| format!("No se pudo abrir la imagen QR: {error}"))?;
    let grayscale = image.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(grayscale);
    let grids = prepared.detect_grids();

    if grids.is_empty() {
        return Err("La imagen no contiene un código QR detectable.".to_owned());
    }

    for grid in grids {
        let (_, payload) = grid
            .decode()
            .map_err(|error| format!("No se pudo decodificar el QR detectado: {error}"))?;

        if payload.trim_start().starts_with("otpauth://") {
            return Ok(payload);
        }
    }

    Err("El QR detectado no contiene un URI otpauth:// compatible.".to_owned())
}
