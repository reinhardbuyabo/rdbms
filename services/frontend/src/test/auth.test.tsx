import { describe, it, expect } from 'vitest';

describe('Utilities', () => {
  it('should have cn utility', async () => {
    const utils = await import('../lib/utils');
    expect(utils.cn).toBeDefined();
    expect(typeof utils.cn).toBe('function');

    expect(utils.cn('class1', 'class2')).toBe('class1 class2');
    expect(utils.cn('class1', { class2: true })).toBe('class1 class2');
    expect(utils.cn('class1', { class2: false })).toBe('class1');
  });

  it('cn should handle multiple inputs', async () => {
    const utils = await import('../lib/utils');
    expect(utils.cn('a', 'b', 'c')).toBe('a b c');
    expect(utils.cn('a', null, 'b')).toBe('a b');
    expect(utils.cn('a', undefined, 'b')).toBe('a b');
  });
});
