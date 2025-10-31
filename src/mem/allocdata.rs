enum AllocType {
    MemMap,
    StackGrowth,
}

pub struct PageInfo {
    va: usize,
    page_type: AllocType,
}
