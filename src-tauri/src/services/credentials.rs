use crate::error::{Result, invalid};
fn entry() -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(
        "studio.quill.desktopchat",
        "openai-api-key",
    )?)
}
pub fn read() -> Result<Option<String>> {
    match entry()?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn write(key: String) -> Result<()> {
    let value = key.trim();
    if value.is_empty() || value.contains(char::is_whitespace) {
        return Err(invalid("Enter an API key without whitespace."));
    }
    Ok(entry()?.set_password(value)?)
}
pub fn remove() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
