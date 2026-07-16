use tokio::task::JoinSet;
use tracing::{info, warn};

use super::model::{MuxOpenedDispatcher, MuxSessionLoop};

pub(crate) async fn run_mux_session_loop<D>(
    request: MuxSessionLoop<'_>,
    tasks: &mut JoinSet<()>,
    dispatcher: &mut D,
) -> Result<(), D::Error>
where
    D: MuxOpenedDispatcher,
{
    info!(
        inbound_tag = request.inbound_tag,
        protocol = request.protocol,
        "mux session started"
    );

    loop {
        if !dispatcher.dispatch_next(tasks).await? {
            break;
        }

        drain_completed_mux_tasks(tasks, request.panic_message);
    }

    if request.abort_on_end {
        tasks.abort_all();
    }

    info!(
        inbound_tag = request.inbound_tag,
        protocol = request.protocol,
        "mux session ended"
    );
    Ok(())
}

pub(crate) fn drain_completed_mux_tasks(tasks: &mut JoinSet<()>, panic_message: &'static str) {
    while let Some(joined) = tasks.try_join_next() {
        if let Err(error) = joined {
            warn!(error = %error, panic_message);
        }
    }
}
