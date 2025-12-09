import { useSearchParams } from 'react-router-dom';
import { Github } from 'lucide-react';

export default function EmbedTopBar() {
  const [searchParams] = useSearchParams();
  const isEmbed = searchParams.has('embed');

  if (!isEmbed) return null;

  return (
    <div className="flex items-center justify-between px-4 h-9 border-b border-white/10 shrink-0">
      <a
        href="https://kevin.rs/projects"
        className="flex items-center gap-2 text-xs text-white/60 hover:text-white transition-colors"
      >
        <span>&larr;</span>
        <span>Back to projects</span>
      </a>
      <a
        href="https://github.com/afterburn/mexchange"
        target="_blank"
        rel="noopener noreferrer"
        className="flex items-center gap-1.5 text-xs text-white/60 hover:text-white transition-colors"
      >
        <Github size={14} />
        <span>View on GitHub</span>
      </a>
    </div>
  );
}
