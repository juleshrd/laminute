interface ToggleSwitchProps {
  checked: boolean;
  disabled?: boolean;
  onChange: (next: boolean) => void;
  "aria-label": string;
}

export function ToggleSwitch({
  checked,
  disabled,
  onChange,
  "aria-label": ariaLabel,
}: ToggleSwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      className={`lm-toggle${checked ? " lm-toggle--on" : ""}`}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    />
  );
}
