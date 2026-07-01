import {clsx} from "clsx"
import * as React from "react"

import styles from "./Input.module.css"

export type InputProps = Readonly<React.InputHTMLAttributes<HTMLInputElement>>

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  (
    {autoComplete = "off", autoCorrect = "off", className, spellCheck = false, ...properties},
    reference,
  ) => (
    <input
      ref={reference}
      className={clsx(styles.input, className)}
      autoComplete={autoComplete}
      autoCorrect={autoCorrect}
      spellCheck={spellCheck}
      {...properties}
    />
  ),
)

Input.displayName = "Input"
