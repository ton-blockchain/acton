import Image from "next/image"
import logoDark from "@/public/logo-dark.svg"
import logoLight from "@/public/logo-light.svg"
import styles from "./ThemeLogo.module.css"

export function ThemeLogo() {
  return (
    <>
      <Image
        alt="Acton"
        src={logoLight}
        width={100}
        height={100}
        sizes="100px"
        className={`h-8 w-auto ${styles.lightLogo}`}
        aria-label="Acton"
      />
      <Image
        alt="Acton"
        src={logoDark}
        width={100}
        height={100}
        sizes="100px"
        className={`h-8 w-auto ${styles.darkLogo}`}
        aria-label="Acton"
      />
    </>
  )
}
