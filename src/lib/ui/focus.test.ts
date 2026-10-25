// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { trapFocus } from './focus';

afterEach(() => {
  document.body.innerHTML = '';
});

describe('trapFocus', () => {
  it('restores focus to the previously focused element on destroy', () => {
    const outside = document.createElement('button');
    document.body.append(outside);
    outside.focus();

    const node = document.createElement('div');
    node.innerHTML = '<button>Inside</button>';
    document.body.append(node);

    const action = trapFocus(node);
    expect(document.activeElement).toBe(node.querySelector('button'));

    action.destroy();
    expect(document.activeElement).toBe(outside);
  });
});
