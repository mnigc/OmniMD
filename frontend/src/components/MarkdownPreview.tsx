import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";

interface MarkdownPreviewProps {
  content: string;
}

export function MarkdownPreview({ content }: MarkdownPreviewProps) {
  if (!content.trim()) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-muted-foreground">
        <div className="w-16 h-16 rounded-full bg-slate-100 flex items-center justify-center mb-3">
          <span className="text-2xl font-bold text-slate-400">{`{ }`}</span>
        </div>
        <p className="text-sm">No markdown content to preview</p>
      </div>
    );
  }

  return (
    <div className="markdown-body">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeRaw]}
        components={{
          code({ className, children, ...props }) {
            const match = /language-(\w+)/.exec(className || "");
            return match ? (
              <pre className="bg-slate-100 border border-slate-200 rounded-md p-3 overflow-x-auto my-2">
                <code
                  className={`font-mono text-sm ${className}`}
                  {...props}
                >
                  {children}
                </code>
              </pre>
            ) : (
              <code
                className="bg-slate-100 px-1.5 py-0.5 rounded text-sm font-mono"
                {...props}
              >
                {children}
              </code>
            );
          },
          pre({ children }) {
            return <div className="my-2">{children}</div>;
          },
          img({ src, alt }) {
            return (
              <img
                src={src}
                alt={alt || ""}
                className="max-w-full h-auto rounded-md border border-slate-200 my-2"
              />
            );
          },
          table({ children }) {
            return (
              <div className="overflow-x-auto my-3 border border-slate-200 rounded-md">
                <table className="min-w-full">{children}</table>
              </div>
            );
          },
          th({ children }) {
            return (
              <th className="px-3 py-2 bg-slate-100 text-left text-sm font-medium border-b border-slate-200">
                {children}
              </th>
            );
          },
          td({ children }) {
            return (
              <td className="px-3 py-2 text-sm border-b border-slate-100 last:border-b-0">
                {children}
              </td>
            );
          },
          a({ href, children }) {
            return (
              <a
                href={href}
                className="text-violet-600 hover:underline"
                target="_blank"
                rel="noopener noreferrer"
              >
                {children}
              </a>
            );
          },
          ul({ children }) {
            return <ul className="list-disc list-inside space-y-1 my-2">{children}</ul>;
          },
          ol({ children }) {
            return <ol className="list-decimal list-inside space-y-1 my-2">{children}</ol>;
          },
          blockquote({ children }) {
            return (
              <blockquote className="border-l-4 border-violet-300 pl-4 my-2 text-slate-600 italic">
                {children}
              </blockquote>
            );
          },
          hr() {
            return <hr className="border-slate-200 my-4" />;
          },
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
