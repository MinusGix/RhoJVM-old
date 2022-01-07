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
        ctx.visit_spans(|span| {
            // Add indentation. This makes it far more readable.
            if idx != 0 {
                write!(writer, "\t")?;
            }
            write!(writer, "{}", span.name())?;
            let ext = span.extensions();

            let fields = &ext.get::<FormattedFields<N>>().expect("will never be None");

            if !fields.is_empty() {
                write!(writer, "{{{}}}", fields)?;
            }
            write!(writer, ": ")?;
            idx += 1;

            Ok(())
        })?;

        ctx.field_format().format_fields(writer, event)?;

        writeln!(writer)
    }
}
