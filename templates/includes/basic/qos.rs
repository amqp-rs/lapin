/// Set the quality of service for this channel.
///
/// Limits the number of unacknowledged messages that the server will deliver.
/// `prefetch_count` controls the maximum number of unacknowledged messages;
/// 0 means no limit. The `global` flag (in [`BasicQosOptions`]) determines
/// whether the limit applies per-consumer (`false`) or across the whole channel
/// (`true`).
///
/// Call this before [`Channel::basic_consume`] to control back-pressure.
///
/// The settings confirmed by the broker are recorded in
/// [`ChannelStatus::qos`](crate::ChannelStatus::qos) and resent when the
/// channel is recovered, before the recovered consumers are recreated, so that
/// they keep their prefetch limit. Consumers you register yourself as soon as
/// recovery completes may still race ahead of that replay; call `basic_qos`
/// again before them if the limit matters.
