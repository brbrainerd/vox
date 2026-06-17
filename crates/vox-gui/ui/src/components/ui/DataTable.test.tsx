// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { DataTable } from './DataTable';

describe('DataTable Component', () => {
  const rows = [
    { id: '1', name: 'Task A', status: 'queued' },
    { id: '2', name: 'Task B', status: 'queued' },
  ];
  const columns = [
    { key: 'id', header: 'ID' },
    { key: 'name', header: 'Name' },
  ];

  it('renders column headers and row cells', () => {
    render(<DataTable rows={rows} columns={columns} getRowId={r => r.id} />);
    expect(screen.getByText('ID')).toBeInTheDocument();
    expect(screen.getByText('Name')).toBeInTheDocument();
    expect(screen.getByText('Task A')).toBeInTheDocument();
  });
});
