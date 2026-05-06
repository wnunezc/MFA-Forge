pub fn decode_otpauth_uri_from_image(path: &str) -> Result<String, String> {
    if path.trim().is_empty() {
        return Err("Provide the path to a QR image before importing.".to_owned());
    }

    let image =
        image::open(path).map_err(|error| format!("The QR image could not be opened: {error}"))?;
    let grayscale = image.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(grayscale);
    let grids = prepared.detect_grids();

    if grids.is_empty() {
        return Err("The image does not contain a detectable QR code.".to_owned());
    }

    for grid in grids {
        let (_, payload) = grid
            .decode()
            .map_err(|error| format!("The detected QR code could not be decoded: {error}"))?;

        if payload.trim_start().starts_with("otpauth://") {
            return Ok(payload);
        }
    }

    Err("The detected QR code does not contain a compatible otpauth:// URI.".to_owned())
}
