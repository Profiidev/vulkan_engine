use std::collections::BTreeMap;

pub struct BufferMemory {
  offset: usize,
  size: usize,
}

pub struct Allocator {
  free: BTreeMap<usize, usize>,
  size: usize,
}

impl BufferMemory {
  pub fn offset(&self) -> usize {
    self.offset
  }

  pub fn size(&self) -> usize {
    self.size
  }
}

impl Allocator {
  pub fn new(size: usize) -> Self {
    let mut free = BTreeMap::new();
    free.insert(0, size);

    Self { free, size }
  }

  pub fn alloc(&mut self, size: usize) -> Option<BufferMemory> {
    let offset = self
      .free
      .iter()
      .find_map(|(o, s)| if *s >= size { Some(*o) } else { None })?;
    let block_size = self.free.remove(&offset)?;

    if block_size > size {
      self.free.insert(offset + size, block_size - size);
    }

    Some(BufferMemory { offset, size })
  }

  pub fn free(&mut self, mem: BufferMemory) {
    let before = self
      .free
      .range(..mem.offset)
      .next_back()
      .filter(|(&o, &s)| o + s == mem.size);
    let after = self.free.range(mem.offset..mem.offset + mem.size).next();

    match (before, after) {
      (Some((&offset, &size)), Some((&a_offset, &a_size))) => {
        self.free.remove(&a_offset);
        self.free.insert(offset, size + mem.size + a_size);
      }
      (None, Some((&a_offset, &a_size))) => {
        self.free.remove(&a_offset);
        self.free.insert(mem.offset, mem.size + a_size);
      }
      (Some((&offset, &size)), None) => {
        self.free.insert(offset, size + mem.size);
      }
      (None, None) => {
        self.free.insert(mem.offset, mem.size);
      }
    }
  }

  pub fn grow(&mut self, additional_size: usize) {
    if let Some((offset, size)) = self.free.iter_mut().next_back() {
      if *offset + *size == self.size {
        *size += additional_size;
        self.size += additional_size;
        return;
      }
    }
    self.free.insert(self.size, additional_size);
    self.size += additional_size;
  }
}
