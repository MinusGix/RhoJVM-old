use tracing::Subscriber;
use tracing_subscriber::{
    fmt::{FormatEvent, FormatFields, FormattedFields},
    registry::LookupSpan,
};

pub(crate) struct Formatter;
impl<S, N> FormatEvent<S, N> for Formatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        writer: &mut dyn std::fmt::Write,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        let level = *event.metadata().level();
        // let target = event.metadata().target();
        write!(writer, "{}: ", level)?;

        let mut idx = 0;
        ctx.visit_spans(|_| {
            // Ignore span names, because for the most part they clog up the log, unfortunately.
            // It would be nice if we could just log the last span, but tracing's api is obtuse..
            write!(writer, ":: ")?;
            idx += 1;

            Ok(())
        })?;

        ctx.field_format().format_fields(writer, event)?;

        writeln!(writer)
    }
}
