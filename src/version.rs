use std::error::Error;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionBump {
    Patch,
    Minor,
    Major,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    InvalidFormat { version: String },
    Overflow { version: String },
}

impl ForgeVersion {
    pub fn bump(&self, bump: VersionBump) -> Result<Self, VersionError> {
        match bump {
            VersionBump::Patch => Ok(Self {
                major: self.major,
                minor: self.minor,
                patch: checked_increment(self.patch, self)?,
            }),
            VersionBump::Minor => Ok(Self {
                major: self.major,
                minor: checked_increment(self.minor, self)?,
                patch: 0,
            }),
            VersionBump::Major => Ok(Self {
                major: checked_increment(self.major, self)?,
                minor: 0,
                patch: 0,
            }),
        }
    }
}

impl fmt::Display for ForgeVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for ForgeVersion {
    type Err = VersionError;

    fn from_str(version: &str) -> Result<Self, Self::Err> {
        let Some(rest) = version.strip_prefix('v') else {
            return Err(invalid(version));
        };
        let parts: Vec<&str> = rest.split('.').collect();
        if parts.len() != 3 {
            return Err(invalid(version));
        }

        let major = parse_part(version, parts[0])?;
        let minor = parse_part(version, parts[1])?;
        let patch = parse_part(version, parts[2])?;

        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for VersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionError::InvalidFormat { version } => write!(
                formatter,
                "invalid version '{version}', expected format vMAJOR.MINOR.PATCH"
            ),
            VersionError::Overflow { version } => {
                write!(
                    formatter,
                    "version '{version}' cannot be incremented safely"
                )
            }
        }
    }
}

impl Error for VersionError {}

pub fn next_version_after(version: &str) -> Result<String, VersionError> {
    next_version_after_with_bump(version, VersionBump::Patch)
}

pub fn next_version_after_with_bump(
    version: &str,
    bump: VersionBump,
) -> Result<String, VersionError> {
    Ok(ForgeVersion::from_str(version)?.bump(bump)?.to_string())
}

fn parse_part(full_version: &str, part: &str) -> Result<u64, VersionError> {
    if part.is_empty()
        || !part.chars().all(|character| character.is_ascii_digit())
        || (part.len() > 1 && part.starts_with('0'))
    {
        return Err(invalid(full_version));
    }

    part.parse::<u64>().map_err(|_| invalid(full_version))
}

fn checked_increment(value: u64, version: &ForgeVersion) -> Result<u64, VersionError> {
    value.checked_add(1).ok_or_else(|| VersionError::Overflow {
        version: version.to_string(),
    })
}

fn invalid(version: &str) -> VersionError {
    VersionError::InvalidFormat {
        version: version.to_string(),
    }
}
