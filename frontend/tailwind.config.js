const config = {
  content: {
    h1: {
      color: "hsl(222.2 47.4% 11.2%)",
      fontWeight: "700",
      fontSize: "3rem",
      lineHeight: "1",
      letterSpacing: "-0.01562em",
    },
    h2: {
      color: "hsl(222.2 47.4% 11.2%)",
      fontWeight: "600",
      fontSize: "1.5rem",
      lineHeight: "2rem",
      letterSpacing: "-0.025em",
    },
    h3: {
      color: "hsl(222.2 47.4% 11.2%)",
      fontWeight: "500",
      fontSize: "1.125rem",
      lineHeight: "1.75rem",
    },
    h4: {
      color: "hsl(222.2 47.4% 11.2%)",
      fontWeight: "500",
      fontSize: "1rem",
      lineHeight: "1.5rem",
    },
    p: {
      color: "hsl(215.4 16.3% 46.9%)",
      fontSize: "0.875rem",
      lineHeight: "1.25rem",
    },
    table: {
      color: "hsl(222.2 47.4% 11.2%)",
      border: "1px solid hsl(214.3 31.8% 91.4%)",
      display: "table",
      width: "100%",
      borderCollapse: "collapse",
      fontSize: "0.875rem",
      lineHeight: "1.25rem",
    },
    thead: {
      display: "table-header-group",
      backgroundColor: "hsl(210 40% 96.1%)",
    },
    th: {
      color: "hsl(222.2 47.4% 11.2%)",
      fontWeight: "500",
      padding: "0.5rem 0.75rem",
      border: "1px solid hsl(214.3 31.8% 91.4%)",
      textAlign: "left",
    },
    td: {
      color: "hsl(215.4 16.3% 46.9%)",
      padding: "0.5rem 0.75rem",
      border: "1px solid hsl(214.3 31.8% 91.4%)",
    },
    tbody: { display: "table-row-group" },
    tr: { display: "table-row" },
    td: { display: "table-cell" },
    th: { display: "table-cell" },
  },
  theme: {
    extend: {
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      keyframes: {
        "accordion-down": {
          from: { height: "0" },
          to: { height: "var(--radix-accordion-content-height)" },
        },
        "accordion-up": {
          from: { height: "var(--radix-accordion-content-height)" },
          to: { height: "0" },
        },
        spin: {
          from: { transform: "rotate(0deg)" },
          to: { transform: "rotate(360deg)" },
        },
        pulse: {
          "0%, 100%": { opacity: "1" },
          "50%": { opacity: "0.5" },
        },
      },
      animation: {
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
        spin: "spin 1s linear infinite",
        pulse: "pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite",
      },
    },
  },
};

export default config;
