// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { Async } from './Async';

describe('<Async>', () => {
  it('renders nothing when status is idle', () => {
    const { container } = render(
      <Async status="idle" data={undefined} error={null}>
        {(d) => <span>{String(d)}</span>}
      </Async>,
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders default loading indicator when status is pending', () => {
    render(
      <Async status="pending" data={undefined} error={null}>
        {(d) => <span>{String(d)}</span>}
      </Async>,
    );
    expect(screen.getByRole('status')).toBeTruthy();
    expect(screen.getByText('Loading…')).toBeTruthy();
  });

  it('renders custom loading slot when renderLoading is provided', () => {
    render(
      <Async
        status="pending"
        data={undefined}
        error={null}
        renderLoading={() => <span>Custom spinner</span>}
      >
        {(d) => <span>{String(d)}</span>}
      </Async>,
    );
    expect(screen.getByText('Custom spinner')).toBeTruthy();
  });

  it('renders default error when status is error', () => {
    render(
      <Async status="error" data={undefined} error={new Error('ipc failed')}>
        {(d) => <span>{String(d)}</span>}
      </Async>,
    );
    expect(screen.getByRole('alert')).toBeTruthy();
    expect(screen.getByText('ipc failed')).toBeTruthy();
  });

  it('renders custom error slot when renderError is provided', () => {
    render(
      <Async
        status="error"
        data={undefined}
        error={new Error('boom')}
        renderError={(e) => <span>Oops: {e.message}</span>}
      >
        {(d) => <span>{String(d)}</span>}
      </Async>,
    );
    expect(screen.getByText('Oops: boom')).toBeTruthy();
  });

  it('renders empty state when data is an empty array', () => {
    render(
      <Async<string[]> status="success" data={[]} error={null}>
        {(items) => <ul>{items.map(i => <li key={i}>{i}</li>)}</ul>}
      </Async>,
    );
    expect(screen.getByText('No results')).toBeTruthy();
  });

  it('renders custom empty slot when renderEmpty is provided', () => {
    render(
      <Async<string[]>
        status="success"
        data={[]}
        error={null}
        renderEmpty={() => <span>Nothing here</span>}
      >
        {(items) => <ul>{items.map(i => <li key={i}>{i}</li>)}</ul>}
      </Async>,
    );
    expect(screen.getByText('Nothing here')).toBeTruthy();
  });

  it('calls children render-prop with data on success', () => {
    render(
      <Async<string> status="success" data="hello" error={null}>
        {(s) => <span>{s}</span>}
      </Async>,
    );
    expect(screen.getByText('hello')).toBeTruthy();
  });

  it('uses custom isEmpty predicate instead of array check', () => {
    const data = { count: 0, items: [] as string[] };
    render(
      <Async<typeof data>
        status="success"
        data={data}
        error={null}
        isEmpty={(d) => d.count === 0}
        renderEmpty={() => <span>Empty via custom predicate</span>}
      >
        {(d) => <span>{d.count} items</span>}
      </Async>,
    );
    expect(screen.getByText('Empty via custom predicate')).toBeTruthy();
  });
});
