use crate::handlers;

#[allow(unused)]
pub struct RelayProcessor {}

impl RelayProcessor {
    #[allow(unused)]
    pub fn new() -> Self {
        Self {}
    }

    #[allow(unused)]
    pub fn place_order() -> anyhow::Result<()> {
        handlers::place_order::handle()?;
        Ok(())
    }

    #[allow(unused)]
    pub fn list_order() -> anyhow::Result<()> {
        handlers::list_orders::handle()?;
        Ok(())
    }

    #[allow(unused)]
    pub fn reply_order() -> anyhow::Result<()> {
        handlers::reply_order::handle()?;
        Ok(())
    }
}
