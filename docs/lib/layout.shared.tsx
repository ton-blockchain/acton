import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import Image from 'next/image';

export const logo = (
    <>
        <Image
            alt="Acton"
            src="/logo.png"
            width={100}
            height={100}
            sizes="100px"
            className="hidden w-22 in-[.uwu]:block"
            aria-label="Acton"
        />
    </>
);

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: (
          <>
            {logo}
            <span className="font-medium in-[.uwu]:hidden">Acton</span>
          </>
      ),
    },
  };
}
