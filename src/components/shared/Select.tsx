// AUTHR_OS — shared Select (R-17): one labeled native select for every
// studio surface. Same inputClass chrome the modals use, plus an optional
// uppercase micro-label and optional muted hint — retires the duplicated
// <span>…<select className={inputClass}> pattern.

import { cn } from '../../lib/utils';
import { inputClass } from '../layout/Modal';

export interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps {
  options: SelectOption[];
  value: string;
  onChange: (value: string) => void;
  /** Uppercase micro-label rendered above the control. */
  label?: string;
  /** Accessible name when no visible label is rendered. */
  ariaLabel?: string;
  className?: string;
  disabled?: boolean;
}

export function Select({ options, value, onChange, label, ariaLabel, className, disabled }: SelectProps) {
  const control = (
    <select
      className={cn(inputClass, 'py-1', className)}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      aria-label={label ?? ariaLabel}
      disabled={disabled}
    >
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );

  if (!label) return control;
  return (
    <label className="grid gap-1">
      <span className="text-2xs text-mut uppercase tracking-wide">{label}</span>
      {control}
    </label>
  );
}
