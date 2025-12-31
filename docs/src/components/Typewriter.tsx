'use client';

import React, {useState, useEffect, CSSProperties} from 'react';

interface TypewriterProps {
  words: string[];
  wait?: number;
  typeSpeed?: number;
  deleteSpeed?: number;
  className?: string;
  style?: CSSProperties;
}

export const Typewriter: React.FC<TypewriterProps> = ({
                                                        words,
                                                        wait = 5000,
                                                        typeSpeed = 100,
                                                        deleteSpeed = 100,
                                                        className = '',
                                                        style,
                                                      }) => {
  const [text, setText] = useState('');
  const [wordIndex, setWordIndex] = useState(0);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isWaiting, setIsWaiting] = useState(false);

  useEffect(() => {
    const currentWord = words[wordIndex];

    if (isWaiting) {
      const timeout = setTimeout(() => {
        setIsWaiting(false);
        setIsDeleting(true);
      }, wait);
      return () => clearTimeout(timeout);
    }

    if (isDeleting) {
      if (text === '') {
        setIsDeleting(false);
        setWordIndex((prev) => (prev + 1) % words.length);
        return;
      }

      const timeout = setTimeout(() => {
        setText(text.slice(0, -1));
      }, deleteSpeed);
      return () => clearTimeout(timeout);
    }

    // Typing
    if (text === currentWord) {
      setIsWaiting(true);
      return;
    }

    const timeout = setTimeout(() => {
      setText(currentWord.slice(0, text.length + 1));
    }, typeSpeed);

    return () => clearTimeout(timeout);
  }, [text, isDeleting, isWaiting, wordIndex, words, wait, typeSpeed, deleteSpeed]);

  return (
    <span className={className} style={style}>
      {text}
      <span className="animate-cursor-blink">|</span>
    </span>
  );
};

export default Typewriter;

