import React, {type ButtonHTMLAttributes} from "react"

import styles from "./ButtonLoader.module.css"

// eslint-disable-next-line functional/type-declaration-immutability
export interface ButtonLoaderProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  readonly children: React.ReactNode
}

const ButtonLoader: React.FC<ButtonLoaderProps> = ({children}) => {
  return (
    <div className={styles.loaderContainer}>
      <div className={styles.loader}></div>
      {children}
    </div>
  )
}

export default ButtonLoader
