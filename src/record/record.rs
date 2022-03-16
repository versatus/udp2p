/// Applied to a type that has a key
pub trait Record {
    type Key;
    fn get_key(&self) -> Self::Key;
}
