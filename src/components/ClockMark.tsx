interface ClockMarkProps {
  className?: string;
}

export function ClockMark({ className = "lm-mark" }: ClockMarkProps) {
  return (
    <span className={className} aria-hidden="true">
      <svg viewBox="0 0 24 24">
        <circle cx="12" cy="13" r="8.2" fill="none" stroke="currentColor" strokeWidth="2.1" />
        <path d="M12 13l3.8-6.3" fill="none" stroke="currentColor" strokeWidth="2.1" />
        <circle cx="12" cy="13" r="1.2" fill="currentColor" />
        <path className="lm-clock-red" d="M11.3 2h1.4v2.1h-1.4z" />
      </svg>
    </span>
  );
}
