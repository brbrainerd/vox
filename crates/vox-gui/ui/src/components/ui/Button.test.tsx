// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { Button } from './Button';

describe('Button', () => {
  it('renders with type="button" by default', () => {
    render(<Button>Click me</Button>);
    expect(screen.getByRole('button')).toHaveAttribute('type', 'button');
  });

  it('forwards aria-label to the underlying element', () => {
    render(<Button aria-label="Close dialog"><span>×</span></Button>);
    expect(screen.getByRole('button', { name: 'Close dialog' })).toBeInTheDocument();
  });

  it('renders children', () => {
    render(<Button>Save</Button>);
    expect(screen.getByText('Save')).toBeInTheDocument();
  });

  it('accepts additional className', () => {
    render(<Button className="my-custom-class">X</Button>);
    expect(screen.getByRole('button')).toHaveClass('my-custom-class');
  });

  it('renders as child element when asChild is set', () => {
    render(
      <Button asChild>
        <a href="/home" role="button">Home</a>
      </Button>
    );
    const link = screen.getByRole('button', { name: 'Home' });
    expect(link.tagName.toLowerCase()).toBe('a');
    expect(link).toHaveAttribute('href', '/home');
  });

  it('allows type to be overridden to "submit"', () => {
    render(<Button type="submit">Submit</Button>);
    expect(screen.getByRole('button')).toHaveAttribute('type', 'submit');
  });

  it('is disabled when disabled prop is set', () => {
    render(<Button disabled>Disabled</Button>);
    expect(screen.getByRole('button')).toBeDisabled();
  });
});
