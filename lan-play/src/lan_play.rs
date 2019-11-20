use crate::proxy::{Proxy, DirectProxy};
use crate::error::Result;

pub struct LanPlay<P> {
    proxy: P,
}

#[async_trait(?Send)]
pub trait LanPlayMain {
    async fn start(&mut self) -> Result<()>;
}

impl<P> LanPlay<P>
where
    P: Proxy + 'static
{
    pub async fn new(proxy: P) -> Result<Box<dyn LanPlayMain>> {
        let ret: Box<dyn LanPlayMain> = Box::new(Self {
            proxy,
        });
        Ok(ret)
    }
}

#[async_trait(?Send)]
impl<P> LanPlayMain for LanPlay<P> {
    async fn start(&mut self) -> Result<()> {
        Ok(())
    }
}
