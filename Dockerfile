FROM ruby:3.4

RUN apt-get update && apt-get install build-essential libclang-dev -y && apt-get clean && rm -rf /var/lib/apt/lists/*
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

COPY pkg/itsi-server-0.2.24.gem .
RUN gem install itsi-server-0.2.24.gem

CMD ["itsi", "serve"]
