import React from "react";
import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import * as SwitchPrimitive from "@radix-ui/react-switch";
import * as SliderPrimitive from "@radix-ui/react-slider";

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  variant?: "primary" | "secondary" | "danger" | "ghost";
}

export function Button({ label, variant = "primary", className = "", ...props }: ButtonProps) {
  const baseStyle = "px-4 py-2 rounded font-medium transition-colors";
  const variants = {
    primary: "bg-blue-600 text-white hover:bg-blue-700",
    secondary: "bg-gray-200 text-gray-900 hover:bg-gray-300",
    danger: "bg-red-600 text-white hover:bg-red-700",
    ghost: "bg-transparent text-blue-600 hover:bg-blue-50"
  };
  return (
    <button className={`${baseStyle} ${variants[variant]} ${className}`} {...props}>
      {label}
    </button>
  );
}

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  onChangeValue?: (val: string) => void;
}

export function Input({ onChangeValue, onChange, className = "", ...props }: InputProps) {
  return (
    <input
      className={`border border-gray-300 rounded px-3 py-2 w-full focus:outline-none focus:ring-2 focus:ring-blue-500 ${className}`}
      onChange={(e) => {
        if (onChangeValue) onChangeValue(e.target.value);
        if (onChange) onChange(e);
      }}
      {...props}
    />
  );
}

export interface TextAreaProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  onChangeValue?: (val: string) => void;
}

export function TextArea({ onChangeValue, onChange, className = "", ...props }: TextAreaProps) {
  return (
    <textarea
      className={`border border-gray-300 rounded px-3 py-2 w-full focus:outline-none focus:ring-2 focus:ring-blue-500 ${className}`}
      onChange={(e) => {
        if (onChangeValue) onChangeValue(e.target.value);
        if (onChange) onChange(e);
      }}
      {...props}
    />
  );
}

export interface SelectProps {
  options: { value: string; label: string }[];
  value: string;
  onChange: (val: string) => void;
  className?: string;
}

export function Select({ options, value, onChange, className = "" }: SelectProps) {
  // A simple <select> for now, wrapped to match Radix's conceptually
  return (
    <select
      className={`border border-gray-300 rounded px-3 py-2 w-full focus:outline-none focus:ring-2 focus:ring-blue-500 bg-white ${className}`}
      value={value}
      onChange={(e) => onChange(e.target.value)}
    >
      {options.map((opt) => (
        <option key={opt.value} value={opt.value}>
          {opt.label}
        </option>
      ))}
    </select>
  );
}

export interface CheckboxProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
  className?: string;
}

export function Checkbox({ checked, onChange, label, className = "" }: CheckboxProps) {
  return (
    <div className={`flex items-center space-x-2 ${className}`}>
      <CheckboxPrimitive.Root
        className="w-5 h-5 rounded border border-gray-300 flex justify-center items-center bg-white data-[state=checked]:bg-blue-600 data-[state=checked]:border-blue-600"
        checked={checked}
        onCheckedChange={(c) => onChange(c === true)}
      >
        <CheckboxPrimitive.Indicator className="text-white">
          <svg width="15" height="15" viewBox="0 0 15 15" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path
              d="M11.4669 3.72684C11.7558 3.91574 11.8369 4.30308 11.648 4.59198L7.39799 11.092C7.29783 11.2452 7.13556 11.3467 6.95402 11.3699C6.77247 11.3931 6.58989 11.3355 6.45446 11.2124L3.70446 8.71241C3.44905 8.48022 3.43023 8.08494 3.66242 7.82953C3.89461 7.57412 4.28989 7.55529 4.5453 7.78749L6.75292 9.79441L10.6018 3.90792C10.7907 3.61902 11.178 3.53795 11.4669 3.72684Z"
              fill="currentColor"
              fillRule="evenodd"
              clipRule="evenodd"
            ></path>
          </svg>
        </CheckboxPrimitive.Indicator>
      </CheckboxPrimitive.Root>
      {label && <label className="text-sm font-medium">{label}</label>}
    </div>
  );
}

export interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  className?: string;
}

export function Switch({ checked, onChange, className = "" }: SwitchProps) {
  return (
    <SwitchPrimitive.Root
      checked={checked}
      onCheckedChange={onChange}
      className={`w-[42px] h-[25px] bg-gray-300 rounded-full relative shadow-inner data-[state=checked]:bg-blue-600 outline-none cursor-default ${className}`}
    >
      <SwitchPrimitive.Thumb className="block w-[21px] h-[21px] bg-white rounded-full shadow transition-transform duration-100 translate-x-0.5 will-change-transform data-[state=checked]:translate-x-[19px]" />
    </SwitchPrimitive.Root>
  );
}

export interface SliderProps {
  min: number;
  max: number;
  value: number;
  onChange: (val: number) => void;
  className?: string;
}

export function Slider({ min, max, value, onChange, className = "" }: SliderProps) {
  return (
    <SliderPrimitive.Root
      className={`relative flex items-center select-none touch-none w-[200px] h-5 ${className}`}
      value={[value]}
      max={max}
      min={min}
      step={1}
      onValueChange={(vals) => onChange(vals[0])}
    >
      <SliderPrimitive.Track className="bg-gray-200 relative grow rounded-full h-[3px]">
        <SliderPrimitive.Range className="absolute bg-blue-600 rounded-full h-full" />
      </SliderPrimitive.Track>
      <SliderPrimitive.Thumb className="block w-5 h-5 bg-white shadow-[0_2px_10px] shadow-black/10 rounded-[10px] hover:bg-gray-50 focus:outline-none focus:shadow-[0_0_0_5px_rgba(0,0,0,0.1)]" />
    </SliderPrimitive.Root>
  );
}
