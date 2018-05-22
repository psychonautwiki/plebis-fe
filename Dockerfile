FROM node:10.1.0-alpine

RUN mkdir -p /usr/src/app
WORKDIR /usr/src/app

COPY package.json /usr/src/app/

RUN npm install && npm cache clean --force

COPY static /usr/src/app/static
COPY templates /usr/src/app/templates

COPY app.js /usr/src/app/

CMD [ "npm", "start" ]