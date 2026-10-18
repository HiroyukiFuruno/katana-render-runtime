pub struct ProbeNames;

impl ProbeNames {
    pub fn scale_name(scale: f32) -> String {
        scale.to_string().replace('.', "_")
    }

    pub fn scroll_name(scroll_y: f32) -> String {
        scroll_y.to_string().replace('.', "_")
    }
}
