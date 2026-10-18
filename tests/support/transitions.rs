use bulletnotes::app::coordinator::{AppAction, AppEffect, AppEvent, Coordinator};
pub trait ReadyTransitions {
    fn dispatch_ready(&mut self, action: AppAction) -> Vec<AppEffect>;
    fn receive_ready(&mut self, event: AppEvent) -> Vec<AppEffect>;
}
fn ready(app: &mut Coordinator, effects: Vec<AppEffect>) -> Vec<AppEffect> {
    effects
        .into_iter()
        .flat_map(|effect| match effect {
            AppEffect::CheckTransition {
                request_id,
                connection,
            } => {
                let next = app.receive(AppEvent::TransitionChecked {
                    request_id,
                    connection,
                    result: Ok(()),
                });
                ready(app, next)
            }
            effect => vec![effect],
        })
        .collect()
}
impl ReadyTransitions for Coordinator {
    fn dispatch_ready(&mut self, action: AppAction) -> Vec<AppEffect> {
        let effects = self.dispatch(action);
        ready(self, effects)
    }
    fn receive_ready(&mut self, event: AppEvent) -> Vec<AppEffect> {
        let effects = self.receive(event);
        ready(self, effects)
    }
}
