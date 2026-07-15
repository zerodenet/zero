#[derive(Debug, Clone, Copy)]
pub struct VmessOutboundOptionsRef<'a> {
    pub id: &'a str,
    pub cipher: &'a str,
    pub mux_concurrency: Option<u32>,
}
