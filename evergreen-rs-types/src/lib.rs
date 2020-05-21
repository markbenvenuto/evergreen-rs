
pub fn make_name(prefix: &str, suffix: &str) -> String {
    if prefix.len() > 0 {
        format!("{}.{}", prefix, suffix)
    } else {
        suffix.to_owned()
    }
}

pub trait EvgFields {
    fn evg_fields_nested(&self, prefix: &str, out: &mut Vec<String>);

    fn evg_fields(&self) -> Vec<String> {
        let mut out : Vec<String> = Vec::new();
        self.evg_fields_nested("", &mut out);
        out
    }

}
