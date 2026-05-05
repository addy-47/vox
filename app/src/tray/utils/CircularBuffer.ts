export class CircularBuffer<T> {
  private buffer: T[];
  private size: number;
  private head: number = 0;

  constructor(size: number) {
    this.size = size;
    this.buffer = [];
  }

  push(item: T) {
    if (this.buffer.length < this.size) {
      this.buffer.push(item);
    } else {
      this.buffer[this.head] = item;
      this.head = (this.head + 1) % this.size;
    }
  }

  getAll(): T[] {
    const result: T[] = [];
    for (let i = 0; i < this.buffer.length; i++) {
      const idx = (this.head + i) % this.buffer.length;
      if (this.buffer[idx] !== undefined) {
        result.push(this.buffer[idx]);
      }
    }
    return result;
  }
}
