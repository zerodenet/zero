/// Common runtime pipe boundary for kernel orchestration.
pub(crate) trait KernelPipe {
    type Input<'a>;
    type Output;
    type Error;

    async fn dispatch(&mut self, input: Self::Input<'_>) -> Result<Self::Output, Self::Error>;
}
