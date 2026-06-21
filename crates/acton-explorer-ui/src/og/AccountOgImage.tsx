export type AccountOgPreview = {
  title: string
  subtitle: string
  shortAddress: string
  rawAddress: string
  status?: string
  type?: string
  detail?: string
  image?: string
  avatarText: string
}

export function AccountOgImage({preview}: {preview: AccountOgPreview}) {
  return (
    <div
      style={{
        position: "relative",
        display: "flex",
        width: "100%",
        height: "100%",
        overflow: "hidden",
        background: "#202020",
        color: "#ffffff",
        fontFamily: "sans serif",
      }}
    >
      <div
        style={{
          position: "absolute",
          inset: 0,
          backgroundImage: "radial-gradient(circle, rgba(255,255,255,0.13) 2px, transparent 3px)",
          backgroundSize: "26px 26px",
          opacity: 0.32,
          transform: "translate(520px, 24px) rotate(-13deg)",
        }}
      />
      <div
        style={{
          position: "absolute",
          right: -140,
          top: -170,
          width: 560,
          height: 560,
          borderRadius: 280,
          background: "rgba(255,255,255,0.035)",
        }}
      />
      <div
        style={{
          position: "absolute",
          left: 90,
          top: 72,
          display: "flex",
          alignItems: "flex-start",
        }}
      >
        <Avatar preview={preview} />
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            marginLeft: 56,
            paddingTop: 8,
            maxWidth: 840,
          }}
        >
          <div
            style={{
              display: "flex",
              color: "#ffffff",
              fontSize: 76,
              fontWeight: 700,
              lineHeight: 1.03,
              letterSpacing: 0,
              whiteSpace: "nowrap",
            }}
          >
            {truncateText(preview.title, 24)}
          </div>
          {(preview.status || preview.type) && (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                marginTop: 26,
                gap: 18,
              }}
            >
              {preview.status && <Badge label={preview.status} variant="success" />}
              {preview.type && <Badge label={preview.type} variant="muted" />}
            </div>
          )}
          {preview.detail && (
            <div
              style={{
                display: "flex",
                marginTop: 30,
                color: "#f0f0f2",
                fontSize: 33,
                fontWeight: 700,
                lineHeight: 1.2,
                whiteSpace: "nowrap",
              }}
            >
              {truncateText(preview.detail, 43)}
            </div>
          )}
        </div>
      </div>
      <div
        style={{
          position: "absolute",
          left: 90,
          bottom: 76,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          height: 62,
          paddingLeft: 30,
          paddingRight: 30,
          borderRadius: 31,
          border: "2px solid #4a4a4d",
          background: "#252527",
          color: "#c8c8ce",
          fontSize: 30,
          fontWeight: 700,
          lineHeight: 1,
          whiteSpace: "nowrap",
        }}
      >
        actonscan.com
      </div>
    </div>
  )
}

export function Avatar({preview}: {preview: AccountOgPreview}) {
  if (preview.image) {
    return (
      <img
        src={preview.image}
        width={152}
        height={152}
        alt=""
        style={{
          width: 152,
          height: 152,
          borderRadius: 76,
          objectFit: "cover",
          background: "#109a9a",
        }}
      />
    )
  }

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        width: 152,
        height: 152,
        borderRadius: 32,
        border: "2px solid #5a5a5d",
        background: "#454547",
      }}
    >
      <svg width="96" height="96" viewBox="0 0 96 96" fill="none">
        <path
          d="M48 18H80L48 76L16 18H48Z"
          stroke="#FFFFFF"
          strokeWidth="8"
          strokeLinejoin="round"
        />
        <path d="M48 20V74" stroke="#FFFFFF" strokeWidth="8" strokeLinecap="round" />
      </svg>
    </div>
  )
}

export function Badge({label, variant}: {label: string; variant: "success" | "muted"}) {
  const isSuccess = variant === "success"
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        height: 52,
        paddingLeft: 22,
        paddingRight: 22,
        borderRadius: 10,
        background: isSuccess ? "#14532d" : "#3a3a3d",
        color: isSuccess ? "#7ee782" : "#f0f0f2",
        fontSize: 31,
        fontWeight: 700,
        lineHeight: 1,
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </div>
  )
}

function truncateText(value: string, maxLength: number) {
  return value.length > maxLength ? `${value.slice(0, maxLength - 1)}…` : value
}
