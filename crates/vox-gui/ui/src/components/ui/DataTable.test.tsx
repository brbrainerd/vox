// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { DataTable } from './DataTable';

describe('DataTable Component', () => {
  const rows = [
    { id: '1', name: 'Task A', status: 'queued', category: 'Backend' },
    { id: '2', name: 'Task B', status: 'in_progress', category: 'Frontend' },
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
    expect(screen.getByText('Task B')).toBeInTheDocument();
  });

  it('supports row selection and bulk actions', async () => {
    const user = userEvent.setup();
    const handleAction = vi.fn();

    render(
      <DataTable
        rows={rows}
        columns={columns}
        getRowId={r => r.id}
        selectable
        onRowAction={handleAction}
      />
    );

    // No rows selected initially
    expect(screen.queryByText(/rows selected/)).not.toBeInTheDocument();

    // Select first row
    const firstCheckbox = screen.getByRole('checkbox', { name: 'Select row 1' });
    await user.click(firstCheckbox);

    // Selected indicator shows
    expect(screen.getByText('1 rows selected')).toBeInTheDocument();

    // Click Pause bulk action
    const pauseBtn = screen.getByRole('button', { name: 'Pause' });
    await user.click(pauseBtn);
    expect(handleAction).toHaveBeenCalledWith('1', 'bulk-pause');

    // Select second row
    const secondCheckbox = screen.getByRole('checkbox', { name: 'Select row 2' });
    await user.click(secondCheckbox);
    expect(screen.getByText('2 rows selected')).toBeInTheDocument();

    // Click Cancel bulk action
    const cancelBtn = screen.getByRole('button', { name: 'Cancel' });
    await user.click(cancelBtn);
    expect(handleAction).toHaveBeenCalledWith('1,2', 'bulk-cancel');
  });

  it('supports toggle select all rows', async () => {
    const user = userEvent.setup();
    render(
      <DataTable
        rows={rows}
        columns={columns}
        getRowId={r => r.id}
        selectable
      />
    );

    const selectAllCheckbox = screen.getByRole('checkbox', { name: 'Select all rows' });
    await user.click(selectAllCheckbox);
    expect(screen.getByText('2 rows selected')).toBeInTheDocument();

    await user.click(selectAllCheckbox);
    expect(screen.queryByText(/rows selected/)).not.toBeInTheDocument();
  });

  it('supports grouping and collapsing groups', async () => {
    const user = userEvent.setup();
    render(
      <DataTable
        rows={rows}
        columns={columns}
        getRowId={r => r.id}
        groupBy={r => r.category}
      />
    );

    // Group headers should exist
    expect(screen.getByText(/Backend \(1\)/)).toBeInTheDocument();
    expect(screen.getByText(/Frontend \(1\)/)).toBeInTheDocument();

    // Both tasks displayed initially
    expect(screen.getByText('Task A')).toBeInTheDocument();
    expect(screen.getByText('Task B')).toBeInTheDocument();

    // Collapse Backend group
    const backendToggle = screen.getByRole('button', { name: /Backend \(1\)/ });
    expect(backendToggle).toHaveAttribute('aria-expanded', 'true');
    await user.click(backendToggle);

    // Task A should no longer be displayed, but Task B remains
    expect(backendToggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByText('Task A')).not.toBeInTheDocument();
    expect(screen.getByText('Task B')).toBeInTheDocument();
  });
});
