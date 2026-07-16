use tokio::task::JoinSet;

pub(crate) struct MuxSessionLoop<'a> {
    pub(crate) inbound_tag: &'a str,
    pub(crate) protocol: &'static str,
    pub(crate) panic_message: &'static str,
    pub(crate) abort_on_end: bool,
}

pub(crate) trait MuxOpenedDispatcher {
    type Error;

    async fn dispatch_next(&mut self, tasks: &mut JoinSet<()>) -> Result<bool, Self::Error>;
}
