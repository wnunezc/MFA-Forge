pub(crate) fn ensure_supported_runtime(surface: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let _ = surface;
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err(format!(
            "{surface} solo está soportado en Windows en esta línea de MFA-Forge. La GUI, el agent local y el servidor MCP dependen de la validación adicional de Windows; fuera de Windows solo se considera soportada la CLI humana."
        ))
    }
}
