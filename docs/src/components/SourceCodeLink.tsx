import {ExternalLink} from 'lucide-react';
import styles from './SourceCodeLink.module.css';

export function SourceCodeLink({href}: { href: string }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer noopener"
      className={styles.link}
    >
      Source code
      <ExternalLink className={styles.icon}/>
    </a>
  );
}
