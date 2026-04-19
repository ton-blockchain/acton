import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { ThemeLogo } from '@/components/ThemeLogo';

export const logo = (
    <ThemeLogo />
);

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: (
          <>
            {logo}
            <span className="text-lg font-medium leading-none in-[.uwu]:hidden">Acton</span>
          </>
      ),
    },
  };
}
