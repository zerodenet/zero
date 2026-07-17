use super::super::ProxyHandle;
use super::runtime::with_current_runtime;

pub(super) fn execute_tun_start(
    handle: &ProxyHandle,
    cmd: &zero_api::TunStartCommand,
) -> zero_api::ApiResult<zero_api::CommandResponse> {
    let proxy = handle.proxy.clone();
    let name = cmd.name.clone();
    let addr = cmd.addr.clone();
    let mask = cmd.mask.clone();
    let mtu = cmd
        .mtu
        .unwrap_or_else(|| proxy.engine().config().runtime.network.mtu);
    let tag = cmd.tag.clone();

    with_current_runtime("no tokio runtime available for TUN command", |rt| {
        rt.block_on(async move {
            proxy
                .start_tun(name.as_deref(), &addr, &mask, mtu, &tag)
                .await
                .map(|_| zero_api::CommandResponse::accepted())
                .map_err(|error| {
                    zero_api::ApiError::new(zero_api::ApiErrorCode::Internal, error.to_string())
                })
        })
    })
}

pub(super) fn execute_tun_stop(
    handle: &ProxyHandle,
) -> zero_api::ApiResult<zero_api::CommandResponse> {
    let proxy = handle.proxy.clone();

    with_current_runtime("no tokio runtime available for TUN command", |rt| {
        rt.block_on(async move {
            proxy
                .stop_tun()
                .map(|_| zero_api::CommandResponse::accepted())
                .map_err(|error| {
                    zero_api::ApiError::new(zero_api::ApiErrorCode::Internal, error.to_string())
                })
        })
    })
}
