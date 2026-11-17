// @generated
impl serde::Serialize for Cancelled {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let len = 0;
        let struct_ser = serializer.serialize_struct("task_runner.v1.Cancelled", len)?;
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Cancelled {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                            Err(serde::de::Error::unknown_field(value, FIELDS))
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Cancelled;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.Cancelled")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Cancelled, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                while map_.next_key::<GeneratedField>()?.is_some() {
                    let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                }
                Ok(Cancelled {
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.Cancelled", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Passed {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let len = 0;
        let struct_ser = serializer.serialize_struct("task_runner.v1.Passed", len)?;
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Passed {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                            Err(serde::de::Error::unknown_field(value, FIELDS))
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Passed;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.Passed")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Passed, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                while map_.next_key::<GeneratedField>()?.is_some() {
                    let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                }
                Ok(Passed {
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.Passed", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for ProcessRecord {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if !self.program.is_empty() {
            len += 1;
        }
        if !self.args.is_empty() {
            len += 1;
        }
        if self.wall_nanos != 0 {
            len += 1;
        }
        if self.startup_nanos.is_some() {
            len += 1;
        }
        if self.exit_code.is_some() {
            len += 1;
        }
        if self.signal.is_some() {
            len += 1;
        }
        if self.killed {
            len += 1;
        }
        if self.stdout_bytes != 0 {
            len += 1;
        }
        if self.stdout_kept != 0 {
            len += 1;
        }
        if self.stderr_bytes != 0 {
            len += 1;
        }
        if self.stderr_kept != 0 {
            len += 1;
        }
        if self.usage.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("task_runner.v1.ProcessRecord", len)?;
        if !self.program.is_empty() {
            struct_ser.serialize_field("program", &self.program)?;
        }
        if !self.args.is_empty() {
            struct_ser.serialize_field("args", &self.args)?;
        }
        if self.wall_nanos != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("wallNanos", ToString::to_string(&self.wall_nanos).as_str())?;
        }
        if let Some(v) = self.startup_nanos.as_ref() {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("startupNanos", ToString::to_string(&v).as_str())?;
        }
        if let Some(v) = self.exit_code.as_ref() {
            struct_ser.serialize_field("exitCode", v)?;
        }
        if let Some(v) = self.signal.as_ref() {
            struct_ser.serialize_field("signal", v)?;
        }
        if self.killed {
            struct_ser.serialize_field("killed", &self.killed)?;
        }
        if self.stdout_bytes != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("stdoutBytes", ToString::to_string(&self.stdout_bytes).as_str())?;
        }
        if self.stdout_kept != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("stdoutKept", ToString::to_string(&self.stdout_kept).as_str())?;
        }
        if self.stderr_bytes != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("stderrBytes", ToString::to_string(&self.stderr_bytes).as_str())?;
        }
        if self.stderr_kept != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("stderrKept", ToString::to_string(&self.stderr_kept).as_str())?;
        }
        if let Some(v) = self.usage.as_ref() {
            struct_ser.serialize_field("usage", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for ProcessRecord {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "program",
            "args",
            "wall_nanos",
            "wallNanos",
            "startup_nanos",
            "startupNanos",
            "exit_code",
            "exitCode",
            "signal",
            "killed",
            "stdout_bytes",
            "stdoutBytes",
            "stdout_kept",
            "stdoutKept",
            "stderr_bytes",
            "stderrBytes",
            "stderr_kept",
            "stderrKept",
            "usage",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Program,
            Args,
            WallNanos,
            StartupNanos,
            ExitCode,
            Signal,
            Killed,
            StdoutBytes,
            StdoutKept,
            StderrBytes,
            StderrKept,
            Usage,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "program" => Ok(GeneratedField::Program),
                            "args" => Ok(GeneratedField::Args),
                            "wallNanos" | "wall_nanos" => Ok(GeneratedField::WallNanos),
                            "startupNanos" | "startup_nanos" => Ok(GeneratedField::StartupNanos),
                            "exitCode" | "exit_code" => Ok(GeneratedField::ExitCode),
                            "signal" => Ok(GeneratedField::Signal),
                            "killed" => Ok(GeneratedField::Killed),
                            "stdoutBytes" | "stdout_bytes" => Ok(GeneratedField::StdoutBytes),
                            "stdoutKept" | "stdout_kept" => Ok(GeneratedField::StdoutKept),
                            "stderrBytes" | "stderr_bytes" => Ok(GeneratedField::StderrBytes),
                            "stderrKept" | "stderr_kept" => Ok(GeneratedField::StderrKept),
                            "usage" => Ok(GeneratedField::Usage),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = ProcessRecord;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.ProcessRecord")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<ProcessRecord, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut program__ = None;
                let mut args__ = None;
                let mut wall_nanos__ = None;
                let mut startup_nanos__ = None;
                let mut exit_code__ = None;
                let mut signal__ = None;
                let mut killed__ = None;
                let mut stdout_bytes__ = None;
                let mut stdout_kept__ = None;
                let mut stderr_bytes__ = None;
                let mut stderr_kept__ = None;
                let mut usage__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Program => {
                            if program__.is_some() {
                                return Err(serde::de::Error::duplicate_field("program"));
                            }
                            program__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Args => {
                            if args__.is_some() {
                                return Err(serde::de::Error::duplicate_field("args"));
                            }
                            args__ = Some(map_.next_value()?);
                        }
                        GeneratedField::WallNanos => {
                            if wall_nanos__.is_some() {
                                return Err(serde::de::Error::duplicate_field("wallNanos"));
                            }
                            wall_nanos__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::StartupNanos => {
                            if startup_nanos__.is_some() {
                                return Err(serde::de::Error::duplicate_field("startupNanos"));
                            }
                            startup_nanos__ = 
                                map_.next_value::<::std::option::Option<::pbjson::private::NumberDeserialize<_>>>()?.map(|x| x.0)
                            ;
                        }
                        GeneratedField::ExitCode => {
                            if exit_code__.is_some() {
                                return Err(serde::de::Error::duplicate_field("exitCode"));
                            }
                            exit_code__ = 
                                map_.next_value::<::std::option::Option<::pbjson::private::NumberDeserialize<_>>>()?.map(|x| x.0)
                            ;
                        }
                        GeneratedField::Signal => {
                            if signal__.is_some() {
                                return Err(serde::de::Error::duplicate_field("signal"));
                            }
                            signal__ = 
                                map_.next_value::<::std::option::Option<::pbjson::private::NumberDeserialize<_>>>()?.map(|x| x.0)
                            ;
                        }
                        GeneratedField::Killed => {
                            if killed__.is_some() {
                                return Err(serde::de::Error::duplicate_field("killed"));
                            }
                            killed__ = Some(map_.next_value()?);
                        }
                        GeneratedField::StdoutBytes => {
                            if stdout_bytes__.is_some() {
                                return Err(serde::de::Error::duplicate_field("stdoutBytes"));
                            }
                            stdout_bytes__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::StdoutKept => {
                            if stdout_kept__.is_some() {
                                return Err(serde::de::Error::duplicate_field("stdoutKept"));
                            }
                            stdout_kept__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::StderrBytes => {
                            if stderr_bytes__.is_some() {
                                return Err(serde::de::Error::duplicate_field("stderrBytes"));
                            }
                            stderr_bytes__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::StderrKept => {
                            if stderr_kept__.is_some() {
                                return Err(serde::de::Error::duplicate_field("stderrKept"));
                            }
                            stderr_kept__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::Usage => {
                            if usage__.is_some() {
                                return Err(serde::de::Error::duplicate_field("usage"));
                            }
                            usage__ = map_.next_value()?;
                        }
                    }
                }
                Ok(ProcessRecord {
                    program: program__.unwrap_or_default(),
                    args: args__.unwrap_or_default(),
                    wall_nanos: wall_nanos__.unwrap_or_default(),
                    startup_nanos: startup_nanos__,
                    exit_code: exit_code__,
                    signal: signal__,
                    killed: killed__.unwrap_or_default(),
                    stdout_bytes: stdout_bytes__.unwrap_or_default(),
                    stdout_kept: stdout_kept__.unwrap_or_default(),
                    stderr_bytes: stderr_bytes__.unwrap_or_default(),
                    stderr_kept: stderr_kept__.unwrap_or_default(),
                    usage: usage__,
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.ProcessRecord", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Resources {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.wall_nanos != 0 {
            len += 1;
        }
        if self.cpu_nanos.is_some() {
            len += 1;
        }
        if !self.processes.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("task_runner.v1.Resources", len)?;
        if self.wall_nanos != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("wallNanos", ToString::to_string(&self.wall_nanos).as_str())?;
        }
        if let Some(v) = self.cpu_nanos.as_ref() {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("cpuNanos", ToString::to_string(&v).as_str())?;
        }
        if !self.processes.is_empty() {
            struct_ser.serialize_field("processes", &self.processes)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Resources {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "wall_nanos",
            "wallNanos",
            "cpu_nanos",
            "cpuNanos",
            "processes",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            WallNanos,
            CpuNanos,
            Processes,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "wallNanos" | "wall_nanos" => Ok(GeneratedField::WallNanos),
                            "cpuNanos" | "cpu_nanos" => Ok(GeneratedField::CpuNanos),
                            "processes" => Ok(GeneratedField::Processes),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Resources;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.Resources")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Resources, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut wall_nanos__ = None;
                let mut cpu_nanos__ = None;
                let mut processes__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::WallNanos => {
                            if wall_nanos__.is_some() {
                                return Err(serde::de::Error::duplicate_field("wallNanos"));
                            }
                            wall_nanos__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::CpuNanos => {
                            if cpu_nanos__.is_some() {
                                return Err(serde::de::Error::duplicate_field("cpuNanos"));
                            }
                            cpu_nanos__ = 
                                map_.next_value::<::std::option::Option<::pbjson::private::NumberDeserialize<_>>>()?.map(|x| x.0)
                            ;
                        }
                        GeneratedField::Processes => {
                            if processes__.is_some() {
                                return Err(serde::de::Error::duplicate_field("processes"));
                            }
                            processes__ = Some(map_.next_value()?);
                        }
                    }
                }
                Ok(Resources {
                    wall_nanos: wall_nanos__.unwrap_or_default(),
                    cpu_nanos: cpu_nanos__,
                    processes: processes__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.Resources", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for RunId {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.millis != 0 {
            len += 1;
        }
        if self.process != 0 {
            len += 1;
        }
        if self.seq != 0 {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("task_runner.v1.RunId", len)?;
        if self.millis != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("millis", ToString::to_string(&self.millis).as_str())?;
        }
        if self.process != 0 {
            struct_ser.serialize_field("process", &self.process)?;
        }
        if self.seq != 0 {
            struct_ser.serialize_field("seq", &self.seq)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for RunId {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "millis",
            "process",
            "seq",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Millis,
            Process,
            Seq,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "millis" => Ok(GeneratedField::Millis),
                            "process" => Ok(GeneratedField::Process),
                            "seq" => Ok(GeneratedField::Seq),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = RunId;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.RunId")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<RunId, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut millis__ = None;
                let mut process__ = None;
                let mut seq__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Millis => {
                            if millis__.is_some() {
                                return Err(serde::de::Error::duplicate_field("millis"));
                            }
                            millis__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::Process => {
                            if process__.is_some() {
                                return Err(serde::de::Error::duplicate_field("process"));
                            }
                            process__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::Seq => {
                            if seq__.is_some() {
                                return Err(serde::de::Error::duplicate_field("seq"));
                            }
                            seq__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                    }
                }
                Ok(RunId {
                    millis: millis__.unwrap_or_default(),
                    process: process__.unwrap_or_default(),
                    seq: seq__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.RunId", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for RunRecord {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.ended.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("task_runner.v1.RunRecord", len)?;
        if let Some(v) = self.ended.as_ref() {
            match v {
                run_record::Ended::Passed(v) => {
                    struct_ser.serialize_field("passed", v)?;
                }
                run_record::Ended::Failed(v) => {
                    struct_ser.serialize_field("failed", v)?;
                }
                run_record::Ended::TimedOut(v) => {
                    struct_ser.serialize_field("timedOut", v)?;
                }
                run_record::Ended::Cancelled(v) => {
                    struct_ser.serialize_field("cancelled", v)?;
                }
                run_record::Ended::Skipped(v) => {
                    struct_ser.serialize_field("skipped", v)?;
                }
                run_record::Ended::Unneeded(v) => {
                    struct_ser.serialize_field("unneeded", v)?;
                }
            }
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for RunRecord {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "passed",
            "failed",
            "timed_out",
            "timedOut",
            "cancelled",
            "skipped",
            "unneeded",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Passed,
            Failed,
            TimedOut,
            Cancelled,
            Skipped,
            Unneeded,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "passed" => Ok(GeneratedField::Passed),
                            "failed" => Ok(GeneratedField::Failed),
                            "timedOut" | "timed_out" => Ok(GeneratedField::TimedOut),
                            "cancelled" => Ok(GeneratedField::Cancelled),
                            "skipped" => Ok(GeneratedField::Skipped),
                            "unneeded" => Ok(GeneratedField::Unneeded),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = RunRecord;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.RunRecord")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<RunRecord, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut ended__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Passed => {
                            if ended__.is_some() {
                                return Err(serde::de::Error::duplicate_field("passed"));
                            }
                            ended__ = map_.next_value::<::std::option::Option<_>>()?.map(run_record::Ended::Passed)
;
                        }
                        GeneratedField::Failed => {
                            if ended__.is_some() {
                                return Err(serde::de::Error::duplicate_field("failed"));
                            }
                            ended__ = map_.next_value::<::std::option::Option<_>>()?.map(run_record::Ended::Failed);
                        }
                        GeneratedField::TimedOut => {
                            if ended__.is_some() {
                                return Err(serde::de::Error::duplicate_field("timedOut"));
                            }
                            ended__ = map_.next_value::<::std::option::Option<_>>()?.map(run_record::Ended::TimedOut)
;
                        }
                        GeneratedField::Cancelled => {
                            if ended__.is_some() {
                                return Err(serde::de::Error::duplicate_field("cancelled"));
                            }
                            ended__ = map_.next_value::<::std::option::Option<_>>()?.map(run_record::Ended::Cancelled)
;
                        }
                        GeneratedField::Skipped => {
                            if ended__.is_some() {
                                return Err(serde::de::Error::duplicate_field("skipped"));
                            }
                            ended__ = map_.next_value::<::std::option::Option<_>>()?.map(run_record::Ended::Skipped);
                        }
                        GeneratedField::Unneeded => {
                            if ended__.is_some() {
                                return Err(serde::de::Error::duplicate_field("unneeded"));
                            }
                            ended__ = map_.next_value::<::std::option::Option<_>>()?.map(run_record::Ended::Unneeded);
                        }
                    }
                }
                Ok(RunRecord {
                    ended: ended__,
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.RunRecord", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for RunReport {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.run.is_some() {
            len += 1;
        }
        if self.outcome.is_some() {
            len += 1;
        }
        if self.seconds != 0. {
            len += 1;
        }
        if self.usage.is_some() {
            len += 1;
        }
        if !self.dir.is_empty() {
            len += 1;
        }
        if !self.invocations.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("task_runner.v1.RunReport", len)?;
        if let Some(v) = self.run.as_ref() {
            struct_ser.serialize_field("run", v)?;
        }
        if let Some(v) = self.outcome.as_ref() {
            struct_ser.serialize_field("outcome", v)?;
        }
        if self.seconds != 0. {
            struct_ser.serialize_field("seconds", &self.seconds)?;
        }
        if let Some(v) = self.usage.as_ref() {
            struct_ser.serialize_field("usage", v)?;
        }
        if !self.dir.is_empty() {
            struct_ser.serialize_field("dir", &self.dir)?;
        }
        if !self.invocations.is_empty() {
            struct_ser.serialize_field("invocations", &self.invocations)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for RunReport {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "run",
            "outcome",
            "seconds",
            "usage",
            "dir",
            "invocations",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Run,
            Outcome,
            Seconds,
            Usage,
            Dir,
            Invocations,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "run" => Ok(GeneratedField::Run),
                            "outcome" => Ok(GeneratedField::Outcome),
                            "seconds" => Ok(GeneratedField::Seconds),
                            "usage" => Ok(GeneratedField::Usage),
                            "dir" => Ok(GeneratedField::Dir),
                            "invocations" => Ok(GeneratedField::Invocations),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = RunReport;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.RunReport")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<RunReport, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut run__ = None;
                let mut outcome__ = None;
                let mut seconds__ = None;
                let mut usage__ = None;
                let mut dir__ = None;
                let mut invocations__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Run => {
                            if run__.is_some() {
                                return Err(serde::de::Error::duplicate_field("run"));
                            }
                            run__ = map_.next_value()?;
                        }
                        GeneratedField::Outcome => {
                            if outcome__.is_some() {
                                return Err(serde::de::Error::duplicate_field("outcome"));
                            }
                            outcome__ = map_.next_value()?;
                        }
                        GeneratedField::Seconds => {
                            if seconds__.is_some() {
                                return Err(serde::de::Error::duplicate_field("seconds"));
                            }
                            seconds__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::Usage => {
                            if usage__.is_some() {
                                return Err(serde::de::Error::duplicate_field("usage"));
                            }
                            usage__ = map_.next_value()?;
                        }
                        GeneratedField::Dir => {
                            if dir__.is_some() {
                                return Err(serde::de::Error::duplicate_field("dir"));
                            }
                            dir__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Invocations => {
                            if invocations__.is_some() {
                                return Err(serde::de::Error::duplicate_field("invocations"));
                            }
                            invocations__ = Some(map_.next_value()?);
                        }
                    }
                }
                Ok(RunReport {
                    run: run__,
                    outcome: outcome__,
                    seconds: seconds__.unwrap_or_default(),
                    usage: usage__,
                    dir: dir__.unwrap_or_default(),
                    invocations: invocations__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.RunReport", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for TaskAttempt {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.id.is_some() {
            len += 1;
        }
        if self.outcome.is_some() {
            len += 1;
        }
        if self.began_unix_nanos.is_some() {
            len += 1;
        }
        if self.slot.is_some() {
            len += 1;
        }
        if self.wait_nanos.is_some() {
            len += 1;
        }
        if self.replayed {
            len += 1;
        }
        if self.output.is_some() {
            len += 1;
        }
        if self.cache_key.is_some() {
            len += 1;
        }
        if self.resources.is_some() {
            len += 1;
        }
        if self.process_record.is_some() {
            len += 1;
        }
        if !self.measures.is_empty() {
            len += 1;
        }
        if !self.evidence.is_empty() {
            len += 1;
        }
        if !self.substeps.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("task_runner.v1.TaskAttempt", len)?;
        if let Some(v) = self.id.as_ref() {
            struct_ser.serialize_field("id", v)?;
        }
        if let Some(v) = self.outcome.as_ref() {
            struct_ser.serialize_field("outcome", v)?;
        }
        if let Some(v) = self.began_unix_nanos.as_ref() {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("beganUnixNanos", ToString::to_string(&v).as_str())?;
        }
        if let Some(v) = self.slot.as_ref() {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("slot", ToString::to_string(&v).as_str())?;
        }
        if let Some(v) = self.wait_nanos.as_ref() {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("waitNanos", ToString::to_string(&v).as_str())?;
        }
        if self.replayed {
            struct_ser.serialize_field("replayed", &self.replayed)?;
        }
        if let Some(v) = self.output.as_ref() {
            struct_ser.serialize_field("output", v)?;
        }
        if let Some(v) = self.cache_key.as_ref() {
            struct_ser.serialize_field("cacheKey", v)?;
        }
        if let Some(v) = self.resources.as_ref() {
            struct_ser.serialize_field("resources", v)?;
        }
        if let Some(v) = self.process_record.as_ref() {
            struct_ser.serialize_field("processRecord", v)?;
        }
        if !self.measures.is_empty() {
            struct_ser.serialize_field("measures", &self.measures)?;
        }
        if !self.evidence.is_empty() {
            struct_ser.serialize_field("evidence", &self.evidence)?;
        }
        if !self.substeps.is_empty() {
            struct_ser.serialize_field("substeps", &self.substeps)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for TaskAttempt {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "id",
            "outcome",
            "began_unix_nanos",
            "beganUnixNanos",
            "slot",
            "wait_nanos",
            "waitNanos",
            "replayed",
            "output",
            "cache_key",
            "cacheKey",
            "resources",
            "process_record",
            "processRecord",
            "measures",
            "evidence",
            "substeps",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Id,
            Outcome,
            BeganUnixNanos,
            Slot,
            WaitNanos,
            Replayed,
            Output,
            CacheKey,
            Resources,
            ProcessRecord,
            Measures,
            Evidence,
            Substeps,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "id" => Ok(GeneratedField::Id),
                            "outcome" => Ok(GeneratedField::Outcome),
                            "beganUnixNanos" | "began_unix_nanos" => Ok(GeneratedField::BeganUnixNanos),
                            "slot" => Ok(GeneratedField::Slot),
                            "waitNanos" | "wait_nanos" => Ok(GeneratedField::WaitNanos),
                            "replayed" => Ok(GeneratedField::Replayed),
                            "output" => Ok(GeneratedField::Output),
                            "cacheKey" | "cache_key" => Ok(GeneratedField::CacheKey),
                            "resources" => Ok(GeneratedField::Resources),
                            "processRecord" | "process_record" => Ok(GeneratedField::ProcessRecord),
                            "measures" => Ok(GeneratedField::Measures),
                            "evidence" => Ok(GeneratedField::Evidence),
                            "substeps" => Ok(GeneratedField::Substeps),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = TaskAttempt;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.TaskAttempt")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<TaskAttempt, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut id__ = None;
                let mut outcome__ = None;
                let mut began_unix_nanos__ = None;
                let mut slot__ = None;
                let mut wait_nanos__ = None;
                let mut replayed__ = None;
                let mut output__ = None;
                let mut cache_key__ = None;
                let mut resources__ = None;
                let mut process_record__ = None;
                let mut measures__ = None;
                let mut evidence__ = None;
                let mut substeps__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Id => {
                            if id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("id"));
                            }
                            id__ = map_.next_value()?;
                        }
                        GeneratedField::Outcome => {
                            if outcome__.is_some() {
                                return Err(serde::de::Error::duplicate_field("outcome"));
                            }
                            outcome__ = map_.next_value()?;
                        }
                        GeneratedField::BeganUnixNanos => {
                            if began_unix_nanos__.is_some() {
                                return Err(serde::de::Error::duplicate_field("beganUnixNanos"));
                            }
                            began_unix_nanos__ = 
                                map_.next_value::<::std::option::Option<::pbjson::private::NumberDeserialize<_>>>()?.map(|x| x.0)
                            ;
                        }
                        GeneratedField::Slot => {
                            if slot__.is_some() {
                                return Err(serde::de::Error::duplicate_field("slot"));
                            }
                            slot__ = 
                                map_.next_value::<::std::option::Option<::pbjson::private::NumberDeserialize<_>>>()?.map(|x| x.0)
                            ;
                        }
                        GeneratedField::WaitNanos => {
                            if wait_nanos__.is_some() {
                                return Err(serde::de::Error::duplicate_field("waitNanos"));
                            }
                            wait_nanos__ = 
                                map_.next_value::<::std::option::Option<::pbjson::private::NumberDeserialize<_>>>()?.map(|x| x.0)
                            ;
                        }
                        GeneratedField::Replayed => {
                            if replayed__.is_some() {
                                return Err(serde::de::Error::duplicate_field("replayed"));
                            }
                            replayed__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Output => {
                            if output__.is_some() {
                                return Err(serde::de::Error::duplicate_field("output"));
                            }
                            output__ = map_.next_value()?;
                        }
                        GeneratedField::CacheKey => {
                            if cache_key__.is_some() {
                                return Err(serde::de::Error::duplicate_field("cacheKey"));
                            }
                            cache_key__ = map_.next_value()?;
                        }
                        GeneratedField::Resources => {
                            if resources__.is_some() {
                                return Err(serde::de::Error::duplicate_field("resources"));
                            }
                            resources__ = map_.next_value()?;
                        }
                        GeneratedField::ProcessRecord => {
                            if process_record__.is_some() {
                                return Err(serde::de::Error::duplicate_field("processRecord"));
                            }
                            process_record__ = map_.next_value()?;
                        }
                        GeneratedField::Measures => {
                            if measures__.is_some() {
                                return Err(serde::de::Error::duplicate_field("measures"));
                            }
                            measures__ = Some(
                                map_.next_value::<std::collections::HashMap<_, ::pbjson::private::NumberDeserialize<f64>>>()?
                                    .into_iter().map(|(k,v)| (k, v.0)).collect()
                            );
                        }
                        GeneratedField::Evidence => {
                            if evidence__.is_some() {
                                return Err(serde::de::Error::duplicate_field("evidence"));
                            }
                            evidence__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Substeps => {
                            if substeps__.is_some() {
                                return Err(serde::de::Error::duplicate_field("substeps"));
                            }
                            substeps__ = Some(map_.next_value()?);
                        }
                    }
                }
                Ok(TaskAttempt {
                    id: id__,
                    outcome: outcome__,
                    began_unix_nanos: began_unix_nanos__,
                    slot: slot__,
                    wait_nanos: wait_nanos__,
                    replayed: replayed__.unwrap_or_default(),
                    output: output__,
                    cache_key: cache_key__,
                    resources: resources__,
                    process_record: process_record__,
                    measures: measures__.unwrap_or_default(),
                    evidence: evidence__.unwrap_or_default(),
                    substeps: substeps__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.TaskAttempt", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for TaskAttemptId {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.invocation.is_some() {
            len += 1;
        }
        if self.attempt != 0 {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("task_runner.v1.TaskAttemptId", len)?;
        if let Some(v) = self.invocation.as_ref() {
            struct_ser.serialize_field("invocation", v)?;
        }
        if self.attempt != 0 {
            struct_ser.serialize_field("attempt", &self.attempt)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for TaskAttemptId {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "invocation",
            "attempt",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Invocation,
            Attempt,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "invocation" => Ok(GeneratedField::Invocation),
                            "attempt" => Ok(GeneratedField::Attempt),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = TaskAttemptId;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.TaskAttemptId")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<TaskAttemptId, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut invocation__ = None;
                let mut attempt__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Invocation => {
                            if invocation__.is_some() {
                                return Err(serde::de::Error::duplicate_field("invocation"));
                            }
                            invocation__ = map_.next_value()?;
                        }
                        GeneratedField::Attempt => {
                            if attempt__.is_some() {
                                return Err(serde::de::Error::duplicate_field("attempt"));
                            }
                            attempt__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                    }
                }
                Ok(TaskAttemptId {
                    invocation: invocation__,
                    attempt: attempt__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.TaskAttemptId", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for TaskInvocation {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.id.is_some() {
            len += 1;
        }
        if !self.task.is_empty() {
            len += 1;
        }
        if !self.attempts.is_empty() {
            len += 1;
        }
        if !self.instances.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("task_runner.v1.TaskInvocation", len)?;
        if let Some(v) = self.id.as_ref() {
            struct_ser.serialize_field("id", v)?;
        }
        if !self.task.is_empty() {
            struct_ser.serialize_field("task", &self.task)?;
        }
        if !self.attempts.is_empty() {
            struct_ser.serialize_field("attempts", &self.attempts)?;
        }
        if !self.instances.is_empty() {
            struct_ser.serialize_field("instances", &self.instances)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for TaskInvocation {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "id",
            "task",
            "attempts",
            "instances",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Id,
            Task,
            Attempts,
            Instances,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "id" => Ok(GeneratedField::Id),
                            "task" => Ok(GeneratedField::Task),
                            "attempts" => Ok(GeneratedField::Attempts),
                            "instances" => Ok(GeneratedField::Instances),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = TaskInvocation;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.TaskInvocation")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<TaskInvocation, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut id__ = None;
                let mut task__ = None;
                let mut attempts__ = None;
                let mut instances__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Id => {
                            if id__.is_some() {
                                return Err(serde::de::Error::duplicate_field("id"));
                            }
                            id__ = map_.next_value()?;
                        }
                        GeneratedField::Task => {
                            if task__.is_some() {
                                return Err(serde::de::Error::duplicate_field("task"));
                            }
                            task__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Attempts => {
                            if attempts__.is_some() {
                                return Err(serde::de::Error::duplicate_field("attempts"));
                            }
                            attempts__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Instances => {
                            if instances__.is_some() {
                                return Err(serde::de::Error::duplicate_field("instances"));
                            }
                            instances__ = Some(map_.next_value()?);
                        }
                    }
                }
                Ok(TaskInvocation {
                    id: id__,
                    task: task__.unwrap_or_default(),
                    attempts: attempts__.unwrap_or_default(),
                    instances: instances__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.TaskInvocation", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for TaskInvocationId {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.run.is_some() {
            len += 1;
        }
        if !self.path.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("task_runner.v1.TaskInvocationId", len)?;
        if let Some(v) = self.run.as_ref() {
            struct_ser.serialize_field("run", v)?;
        }
        if !self.path.is_empty() {
            struct_ser.serialize_field("path", &self.path)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for TaskInvocationId {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "run",
            "path",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Run,
            Path,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "run" => Ok(GeneratedField::Run),
                            "path" => Ok(GeneratedField::Path),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = TaskInvocationId;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.TaskInvocationId")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<TaskInvocationId, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut run__ = None;
                let mut path__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Run => {
                            if run__.is_some() {
                                return Err(serde::de::Error::duplicate_field("run"));
                            }
                            run__ = map_.next_value()?;
                        }
                        GeneratedField::Path => {
                            if path__.is_some() {
                                return Err(serde::de::Error::duplicate_field("path"));
                            }
                            path__ = Some(map_.next_value()?);
                        }
                    }
                }
                Ok(TaskInvocationId {
                    run: run__,
                    path: path__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.TaskInvocationId", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for TaskSubstep {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if !self.name.is_empty() {
            len += 1;
        }
        if self.outcome.is_some() {
            len += 1;
        }
        if self.began_unix_nanos.is_some() {
            len += 1;
        }
        if self.resources.is_some() {
            len += 1;
        }
        if !self.measures.is_empty() {
            len += 1;
        }
        if !self.evidence.is_empty() {
            len += 1;
        }
        if !self.substeps.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("task_runner.v1.TaskSubstep", len)?;
        if !self.name.is_empty() {
            struct_ser.serialize_field("name", &self.name)?;
        }
        if let Some(v) = self.outcome.as_ref() {
            struct_ser.serialize_field("outcome", v)?;
        }
        if let Some(v) = self.began_unix_nanos.as_ref() {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("beganUnixNanos", ToString::to_string(&v).as_str())?;
        }
        if let Some(v) = self.resources.as_ref() {
            struct_ser.serialize_field("resources", v)?;
        }
        if !self.measures.is_empty() {
            struct_ser.serialize_field("measures", &self.measures)?;
        }
        if !self.evidence.is_empty() {
            struct_ser.serialize_field("evidence", &self.evidence)?;
        }
        if !self.substeps.is_empty() {
            struct_ser.serialize_field("substeps", &self.substeps)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for TaskSubstep {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "name",
            "outcome",
            "began_unix_nanos",
            "beganUnixNanos",
            "resources",
            "measures",
            "evidence",
            "substeps",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Name,
            Outcome,
            BeganUnixNanos,
            Resources,
            Measures,
            Evidence,
            Substeps,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "name" => Ok(GeneratedField::Name),
                            "outcome" => Ok(GeneratedField::Outcome),
                            "beganUnixNanos" | "began_unix_nanos" => Ok(GeneratedField::BeganUnixNanos),
                            "resources" => Ok(GeneratedField::Resources),
                            "measures" => Ok(GeneratedField::Measures),
                            "evidence" => Ok(GeneratedField::Evidence),
                            "substeps" => Ok(GeneratedField::Substeps),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = TaskSubstep;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.TaskSubstep")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<TaskSubstep, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut name__ = None;
                let mut outcome__ = None;
                let mut began_unix_nanos__ = None;
                let mut resources__ = None;
                let mut measures__ = None;
                let mut evidence__ = None;
                let mut substeps__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Name => {
                            if name__.is_some() {
                                return Err(serde::de::Error::duplicate_field("name"));
                            }
                            name__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Outcome => {
                            if outcome__.is_some() {
                                return Err(serde::de::Error::duplicate_field("outcome"));
                            }
                            outcome__ = map_.next_value()?;
                        }
                        GeneratedField::BeganUnixNanos => {
                            if began_unix_nanos__.is_some() {
                                return Err(serde::de::Error::duplicate_field("beganUnixNanos"));
                            }
                            began_unix_nanos__ = 
                                map_.next_value::<::std::option::Option<::pbjson::private::NumberDeserialize<_>>>()?.map(|x| x.0)
                            ;
                        }
                        GeneratedField::Resources => {
                            if resources__.is_some() {
                                return Err(serde::de::Error::duplicate_field("resources"));
                            }
                            resources__ = map_.next_value()?;
                        }
                        GeneratedField::Measures => {
                            if measures__.is_some() {
                                return Err(serde::de::Error::duplicate_field("measures"));
                            }
                            measures__ = Some(
                                map_.next_value::<std::collections::HashMap<_, ::pbjson::private::NumberDeserialize<f64>>>()?
                                    .into_iter().map(|(k,v)| (k, v.0)).collect()
                            );
                        }
                        GeneratedField::Evidence => {
                            if evidence__.is_some() {
                                return Err(serde::de::Error::duplicate_field("evidence"));
                            }
                            evidence__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Substeps => {
                            if substeps__.is_some() {
                                return Err(serde::de::Error::duplicate_field("substeps"));
                            }
                            substeps__ = Some(map_.next_value()?);
                        }
                    }
                }
                Ok(TaskSubstep {
                    name: name__.unwrap_or_default(),
                    outcome: outcome__,
                    began_unix_nanos: began_unix_nanos__,
                    resources: resources__,
                    measures: measures__.unwrap_or_default(),
                    evidence: evidence__.unwrap_or_default(),
                    substeps: substeps__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.TaskSubstep", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for TimedOut {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let len = 0;
        let struct_ser = serializer.serialize_struct("task_runner.v1.TimedOut", len)?;
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for TimedOut {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                            Err(serde::de::Error::unknown_field(value, FIELDS))
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = TimedOut;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.TimedOut")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<TimedOut, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                while map_.next_key::<GeneratedField>()?.is_some() {
                    let _ = map_.next_value::<serde::de::IgnoredAny>()?;
                }
                Ok(TimedOut {
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.TimedOut", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Usage {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if self.user_nanos != 0 {
            len += 1;
        }
        if self.system_nanos != 0 {
            len += 1;
        }
        if self.max_rss_bytes != 0 {
            len += 1;
        }
        if self.minor_faults != 0 {
            len += 1;
        }
        if self.major_faults != 0 {
            len += 1;
        }
        if self.in_blocks != 0 {
            len += 1;
        }
        if self.out_blocks != 0 {
            len += 1;
        }
        if self.voluntary_switches != 0 {
            len += 1;
        }
        if self.involuntary_switches != 0 {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("task_runner.v1.Usage", len)?;
        if self.user_nanos != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("userNanos", ToString::to_string(&self.user_nanos).as_str())?;
        }
        if self.system_nanos != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("systemNanos", ToString::to_string(&self.system_nanos).as_str())?;
        }
        if self.max_rss_bytes != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("maxRssBytes", ToString::to_string(&self.max_rss_bytes).as_str())?;
        }
        if self.minor_faults != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("minorFaults", ToString::to_string(&self.minor_faults).as_str())?;
        }
        if self.major_faults != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("majorFaults", ToString::to_string(&self.major_faults).as_str())?;
        }
        if self.in_blocks != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("inBlocks", ToString::to_string(&self.in_blocks).as_str())?;
        }
        if self.out_blocks != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("outBlocks", ToString::to_string(&self.out_blocks).as_str())?;
        }
        if self.voluntary_switches != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("voluntarySwitches", ToString::to_string(&self.voluntary_switches).as_str())?;
        }
        if self.involuntary_switches != 0 {
            #[allow(clippy::needless_borrow)]
            #[allow(clippy::needless_borrows_for_generic_args)]
            struct_ser.serialize_field("involuntarySwitches", ToString::to_string(&self.involuntary_switches).as_str())?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Usage {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "user_nanos",
            "userNanos",
            "system_nanos",
            "systemNanos",
            "max_rss_bytes",
            "maxRssBytes",
            "minor_faults",
            "minorFaults",
            "major_faults",
            "majorFaults",
            "in_blocks",
            "inBlocks",
            "out_blocks",
            "outBlocks",
            "voluntary_switches",
            "voluntarySwitches",
            "involuntary_switches",
            "involuntarySwitches",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            UserNanos,
            SystemNanos,
            MaxRssBytes,
            MinorFaults,
            MajorFaults,
            InBlocks,
            OutBlocks,
            VoluntarySwitches,
            InvoluntarySwitches,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "userNanos" | "user_nanos" => Ok(GeneratedField::UserNanos),
                            "systemNanos" | "system_nanos" => Ok(GeneratedField::SystemNanos),
                            "maxRssBytes" | "max_rss_bytes" => Ok(GeneratedField::MaxRssBytes),
                            "minorFaults" | "minor_faults" => Ok(GeneratedField::MinorFaults),
                            "majorFaults" | "major_faults" => Ok(GeneratedField::MajorFaults),
                            "inBlocks" | "in_blocks" => Ok(GeneratedField::InBlocks),
                            "outBlocks" | "out_blocks" => Ok(GeneratedField::OutBlocks),
                            "voluntarySwitches" | "voluntary_switches" => Ok(GeneratedField::VoluntarySwitches),
                            "involuntarySwitches" | "involuntary_switches" => Ok(GeneratedField::InvoluntarySwitches),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Usage;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct task_runner.v1.Usage")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Usage, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut user_nanos__ = None;
                let mut system_nanos__ = None;
                let mut max_rss_bytes__ = None;
                let mut minor_faults__ = None;
                let mut major_faults__ = None;
                let mut in_blocks__ = None;
                let mut out_blocks__ = None;
                let mut voluntary_switches__ = None;
                let mut involuntary_switches__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::UserNanos => {
                            if user_nanos__.is_some() {
                                return Err(serde::de::Error::duplicate_field("userNanos"));
                            }
                            user_nanos__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::SystemNanos => {
                            if system_nanos__.is_some() {
                                return Err(serde::de::Error::duplicate_field("systemNanos"));
                            }
                            system_nanos__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::MaxRssBytes => {
                            if max_rss_bytes__.is_some() {
                                return Err(serde::de::Error::duplicate_field("maxRssBytes"));
                            }
                            max_rss_bytes__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::MinorFaults => {
                            if minor_faults__.is_some() {
                                return Err(serde::de::Error::duplicate_field("minorFaults"));
                            }
                            minor_faults__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::MajorFaults => {
                            if major_faults__.is_some() {
                                return Err(serde::de::Error::duplicate_field("majorFaults"));
                            }
                            major_faults__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::InBlocks => {
                            if in_blocks__.is_some() {
                                return Err(serde::de::Error::duplicate_field("inBlocks"));
                            }
                            in_blocks__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::OutBlocks => {
                            if out_blocks__.is_some() {
                                return Err(serde::de::Error::duplicate_field("outBlocks"));
                            }
                            out_blocks__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::VoluntarySwitches => {
                            if voluntary_switches__.is_some() {
                                return Err(serde::de::Error::duplicate_field("voluntarySwitches"));
                            }
                            voluntary_switches__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::InvoluntarySwitches => {
                            if involuntary_switches__.is_some() {
                                return Err(serde::de::Error::duplicate_field("involuntarySwitches"));
                            }
                            involuntary_switches__ = 
                                Some(map_.next_value::<::pbjson::private::NumberDeserialize<_>>()?.0)
                            ;
                        }
                    }
                }
                Ok(Usage {
                    user_nanos: user_nanos__.unwrap_or_default(),
                    system_nanos: system_nanos__.unwrap_or_default(),
                    max_rss_bytes: max_rss_bytes__.unwrap_or_default(),
                    minor_faults: minor_faults__.unwrap_or_default(),
                    major_faults: major_faults__.unwrap_or_default(),
                    in_blocks: in_blocks__.unwrap_or_default(),
                    out_blocks: out_blocks__.unwrap_or_default(),
                    voluntary_switches: voluntary_switches__.unwrap_or_default(),
                    involuntary_switches: involuntary_switches__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("task_runner.v1.Usage", FIELDS, GeneratedVisitor)
    }
}
