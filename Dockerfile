FROM lambci/lambda:build-ruby2.7

ENV AWS_DEFAULT_REGION us-west-2

COPY Gemfile Gemfile
COPY Gemfile.lock Gemfile.lock
RUN bundle install --deployment --without=development

COPY . .

RUN zip -9yr lambda.zip .

CMD aws lambda update-function-code --function-name HNDigest --zip-file fileb://lambda.zip
