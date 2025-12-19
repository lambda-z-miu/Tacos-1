#[derive(Clone)]
pub enum AllocType {
    MemMap,
    Stack,
    Init,
    Heap,
}

#[derive(Clone)]
pub struct PageInfo {
    pub va: usize,
    pub page_type: AllocType,
}

/*
impl PageInfo {
    fn print(&self) {
        match  {

        }
        kprintln!("{}")
    }
}*/
