FROM rust:1.74 as builder
RUN USER=root cargo new --bin image-tag-constraint-controller

WORKDIR /image-tag-constraint-controller
RUN cargo install cargo-chef 

# COPY ./Cargo.toml ./Cargo.toml
# RUN cargo build --release
# RUN rm src/*.rs

COPY . ./
RUN cargo chef prepare  --recipe-path recipe.json


# RUN cargo clean
# RUN cargo build --release

FROM rust:1.74 as cacher
WORKDIR /image-tag-constraint-controller
RUN cargo install cargo-chef
COPY --from=builder /image-tag-constraint-controller/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json


FROM rust:1.74 as planner
WORKDIR /image-tag-constraint-controller
COPY . .
# Copy over the cached dependencies
COPY --from=cacher /image-tag-constraint-controller/target target
COPY --from=cacher $CARGO_HOME $CARGO_HOME
RUN cargo build  --release --bin image-tag-constraint-controller

FROM debian:buster-slim
ARG APP=/usr/src/app
WORKDIR ${APP}

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

EXPOSE 8443

ENV TZ=Etc/UTC \
    APP_USER=appuser

RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER \
    && mkdir -p ${APP}

COPY --from=planner /image-tag-constraint-controller/target/release/image-tag-constraint-controller ${APP}/image-tag-constraint-controller

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER

CMD ["./image-tag-constraint-controller"]