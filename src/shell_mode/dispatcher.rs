#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchAction {
    Continue,
    RouteToAgent,
    OfferHint,
}

pub fn classify(exit_code: Option<i32>) -> DispatchAction {
    match exit_code {
        Some(0) => DispatchAction::Continue,
        Some(127) => DispatchAction::RouteToAgent,
        _ => DispatchAction::OfferHint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_exit_continues() {
        assert_eq!(classify(Some(0)), DispatchAction::Continue);
    }

    #[test]
    fn one_two_seven_routes_to_agent() {
        assert_eq!(classify(Some(127)), DispatchAction::RouteToAgent);
    }

    #[test]
    fn other_non_zero_offers_hint() {
        assert_eq!(classify(Some(1)), DispatchAction::OfferHint);
        assert_eq!(classify(Some(2)), DispatchAction::OfferHint);
        assert_eq!(classify(Some(130)), DispatchAction::OfferHint);
    }

    #[test]
    fn signal_offers_hint() {
        assert_eq!(classify(None), DispatchAction::OfferHint);
    }
}
