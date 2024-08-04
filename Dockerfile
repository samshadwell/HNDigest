FROM public.ecr.aws/sam/build-ruby3.3:latest-x86_64
WORKDIR /var/task

RUN gem update bundler

ENV AWS_DEFAULT_REGION us-west-2

COPY Gemfile .
COPY Gemfile.lock .

RUN bundle config set --local deployment 'true'
RUN bundle config set --local without 'development'
RUN bundle config set --local path 'vendor/bundle'
RUN bundle install

COPY . .

RUN zip -9yr lambda.zip .

CMD aws lambda update-function-code --function-name HNDigest --zip-file fileb://lambda.zip
